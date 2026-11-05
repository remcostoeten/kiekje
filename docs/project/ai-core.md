---
title: AI core proposal
description: A proposal for an opt-in, vendor-agnostic AI layer with typed adapters, live model lists and API keys kept in the desktop keyring.
---

# AI core proposal

Status: proposal, nothing built. Text capture with Tesseract ships first and stays the default. This page describes the layer that would let a cloud model read or describe a capture instead, and what has to be decided before it is built.

## Requirements

- Off by default. Nothing is sent anywhere until the user turns the feature on and picks a vendor and a model.
- One core with an adapter per vendor: Anthropic, OpenAI, DeepSeek, Zhipu GLM, Moonshot and OpenRouter. Adding a vendor is a new adapter value, not a change to the callers.
- Typed from end to end. Vendors, protocols, capabilities, errors and states are enums; model ids and API keys are newtypes; no JSON reaches product code.
- The model picker lists the current models of the chosen vendor and accepts a model id typed by hand for models Kiekje does not list yet.
- The first time a vendor is chosen, Kiekje asks for that vendor's API key, checks it, and stores it in the desktop keyring. The key never lands in `config.toml`, a log line, a notification or a crash report.
- The storage and the data flow hold up in an ISO 27001 audit of the organisation that runs Kiekje.

## Shape

```
src/ai/
  mod.rs        public API: Task, Answer, AiError, run()
  vendor.rs     Vendor enum and its static descriptor
  protocol/     anthropic.rs (Messages API), openai.rs (chat completions)
  models.rs     catalog: curated defaults, live list, cache, custom ids
  keys.rs       ApiKey newtype and the keyring store
  audit.rs      local log of every request
```

It stays a module in the one binary. The boundary is the public API in `mod.rs`: product code builds a `Task`, calls `ai::run` on a worker thread and gets an `Answer` or a typed `AiError` back over `async-channel`. Nothing outside `src/ai` sees a vendor, a URL or a key.

### Vendors are data, protocols are code

Five of the six vendors speak the OpenAI chat completions format. Anthropic speaks its own Messages API. So there are two protocol implementations and six vendor descriptors:

```rust
pub enum Vendor {
    Anthropic,
    OpenAi,
    DeepSeek,
    Zhipu,
    Moonshot,
    OpenRouter,
}

pub enum Protocol {
    AnthropicMessages,
    OpenAiChat,
}

pub struct Descriptor {
    pub name: &'static str,
    pub base_url: &'static str,
    pub protocol: Protocol,
    pub auth: Auth,
    pub keys_page: &'static str,
    pub models: ModelSource,
}

impl Vendor {
    pub const ALL: [Self; 6] = [/* ... */];
    pub fn descriptor(self) -> &'static Descriptor { /* match */ }
}
```

`Auth` is `XApiKey` for Anthropic and `Bearer` for the rest. `ModelSource` says whether the vendor has a model list endpoint. A seventh vendor that speaks either protocol is one more variant and one more descriptor; the compiler then points at every `match` that has to handle it.

Each protocol implements one trait:

```rust
trait Adapter {
    fn send(&self, vendor: &Descriptor, key: &ApiKey, model: &ModelId, task: &Task)
        -> Result<Answer, AiError>;
    fn list_models(&self, vendor: &Descriptor, key: &ApiKey) -> Result<Vec<Model>, AiError>;
}
```

The request and response bodies are private `serde` structs inside each protocol file, parsed at the boundary into `Answer` and `Model`. HTTP goes through `ureq` with rustls, which Kiekje already uses for the update check and the upload, with a connect and read timeout and no retries hidden from the caller.

### Tasks and capabilities

```rust
pub enum Task {
    ReadText { image: Png, languages: Vec<Language> },
}

pub enum Capability {
    Text,
    Vision,
}
```

Text capture is the first task and needs `Vision`. Later tasks (describe a capture for alt text, name a file) are new variants. A model without the capability a task needs is listed but disabled for that task, with the reason.

### Models

The picker merges three sources:

1. A curated list per vendor in code, with the models Kiekje recommends for each task. It is short and is reviewed on every release.
2. The vendor's live list from its models endpoint, fetched with the stored key, cached for a day in `$XDG_CACHE_HOME/kiekje/models.json` and refreshed in the background. OpenRouter's list also carries input modalities and prices, so capability and cost come from it directly.
3. A model id typed by hand. It is stored as written, marked "not checked", and used as is; a vendor error on first use comes back as `AiError::Model` with the vendor's message.

```rust
pub struct ModelId(String);

pub struct Model {
    pub id: ModelId,
    pub name: String,
    pub capabilities: Vec<Capability>,
    pub origin: Origin,
}

pub enum Origin {
    Curated,
    Listed,
    Custom,
}
```

Which models each vendor has with image input changes often. The curated lists are written when the adapters are built, from each vendor's documentation at that time, not from this page.

### Errors

```rust
pub enum AiError {
    Disabled,
    NoKey(Vendor),
    KeyRejected(Vendor),
    Keyring(KeyringError),
    RateLimited { retry_after: Option<Duration> },
    Model(String),
    MissingCapability(Capability),
    Network(String),
    Vendor { status: u16, message: String },
    Malformed(String),
}
```

`KeyRejected` reopens the key dialog. `RateLimited` and `Network` surface in the notification. For text capture any `AiError` can fall back to Tesseract when it is installed, with the reason logged.

## API keys

### Where they live

Keys go into the user's keyring through the freedesktop Secret Service API (`org.freedesktop.secrets`), which KWallet, GNOME Keyring and KeePassXC all implement. The client is the `oo7` crate, which talks D-Bus through `zbus` like the rest of Kiekje and also covers the Secret portal, so a later Flatpak build keeps working. Each key is one item:

| Field | Value |
| --- | --- |
| Label | `Kiekje: Anthropic API key` |
| `application` | `dev.remcostoeten.Kiekje` |
| `vendor` | `anthropic` |

The keyring encrypts the item at rest and unlocks with the user's login. Kiekje reads the key just before a request and drops it after.

There is no fallback to a file. On a session without a Secret Service provider the AI feature reports as unavailable with the reason, `doctor` names the fix (install and start one), and Tesseract keeps working. This follows the rule that an unimplemented capability is reported as unavailable instead of simulated.

### In memory

```rust
pub struct ApiKey(SecretString);
```

`SecretString` from the `secrecy` crate zeroes the memory on drop and prints `[REDACTED]` through `Debug`. `ApiKey` has no `Display`, no `Serialize` and no `Clone`, so the compiler refuses to log it, write it to the config or copy it around. The only way to read it is `expose_secret()` inside the two protocol files, where it becomes the auth header.

### The first-use flow

1. The user turns AI on and picks a vendor and a model in Settings.
2. No key for that vendor is in the keyring, so a dialog asks for it, with a masked entry and a link to the vendor's keys page.
3. Kiekje checks the key with the cheapest authenticated call the vendor has (its models endpoint) on a worker thread.
4. A key that works is written to the keyring; one that does not stays in the dialog with the vendor's message.
5. Settings shows "Key stored in KWallet" (or the provider's name) with Replace and Remove buttons. Remove deletes the keyring item.

## Data flow and audit

A request carries the cropped region as PNG and a fixed prompt, nothing else: no file name, path, window title or other capture. The answer comes back as text and goes to the clipboard like a Tesseract read.

Every request appends one line to `$XDG_STATE_HOME/kiekje/ai.log`: the time, the vendor, the model, the task, the bytes sent and received, and the outcome. It never holds the image, the text or the key. The About page links the log and can clear it, and the privacy page lists the vendors as network destinations next to the update check and the upload.

How this maps to ISO 27001 Annex A, for an organisation that has to account for the tools its people run:

| Control | What Kiekje does |
| --- | --- |
| 5.12, 5.34 classification, privacy and PII | Off by default, one region per request, nothing else attached |
| 5.14 information transfer | Vendors listed on the privacy page; TLS only, certificate checks on |
| 5.17 authentication information | Keys only in the session keyring, entered by the user, removable |
| 8.5 secure authentication | The keyring unlocks with the user's login; Kiekje never asks for that password |
| 8.12 data leakage prevention | `ApiKey` cannot be logged or serialised; keys never in config, logs or notifications |
| 8.15 logging | Local request log without content |
| 8.24 use of cryptography | Encryption at rest by the keyring, TLS 1.2 or later in transit through rustls |

Kiekje itself is not certified; this makes it possible to show an auditor where keys are, what leaves the machine and when.

## Configuration

```toml
[ai]
enabled = false
vendor = "anthropic"
model = "claude-haiku-5-5"

[text]
engine = "tesseract"
languages = ["eng"]
```

`[text] engine = "ai"` sends text capture to the chosen model. The `[ai]` section holds no secrets and is safe to share.

## Phases

1. Core, the OpenAI chat adapter, the Anthropic adapter, the keyring store with its dialog, the audit log, and text capture as the first task with Tesseract as fallback.
2. The remaining vendors as descriptors, live model lists and the cache, custom model ids.
3. More tasks once text capture has been used for a while.

## Open questions

- Should each send be confirmed the first time per session, or is the opt-in switch enough?
- Should prices show next to models where the vendor reports them (OpenRouter does), or stay out of the UI?
- Should the AI engine run only when Tesseract is missing or unsure, or replace it entirely when chosen?
