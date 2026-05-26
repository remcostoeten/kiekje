#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
exec cargo run -q -- --edit --annotate --copy --preview
