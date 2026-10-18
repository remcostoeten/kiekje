# AUR publishing

Every Release run publishes both `kiekje` and `kiekje-bin` after the GitHub release completes: a pushed `v*` tag, or running the Release workflow by hand. Running it with `republish` on an existing tag rebuilds the assets and publishes that version again; the AUR push is skipped when the PKGBUILDs did not change.

The repository needs an `AUR_SSH_PRIVATE_KEY` Actions secret containing the private key whose public half is registered on the AUR account that owns `kiekje`. The key is used only by the release job to clone and push the AUR Git repository.

