import { useEffect, useState } from "react";

import { track } from "@/shared/helpers/track";

import { Icon } from "./icons";

type Props = {
  text: string;
  method: string;
};

export function CopyButton({ text, method }: Props) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1600);
    return () => window.clearTimeout(timer);
  }, [copied]);

  function copy() {
    navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopied(true);
        track("install_copy", { method });
      })
      .catch(() => {
        // Without clipboard permission the command stays selectable in the block.
      });
  }

  return (
    <button
      type="button"
      className="kk-copy"
      data-copied={copied}
      onClick={copy}
      aria-label={copied ? "Copied" : "Copy command"}
    >
      <Icon name={copied ? "check" : "copy"} size={16} />
      <span aria-live="polite">{copied ? "Copied" : "Copy"}</span>
    </button>
  );
}
