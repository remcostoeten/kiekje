# AUR publishing

Tagged releases publish both `kiekje` and `kiekje-bin` after the GitHub release completes.

The repository needs an `AUR_SSH_PRIVATE_KEY` Actions secret containing the private key whose public half is registered on the AUR account that owns `kiekje`. The key is used only by the release job to clone and push the AUR Git repository.

