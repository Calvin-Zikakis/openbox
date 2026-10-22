#!/usr/bin/env bash
# Downloads the neural beat models openbox uses into models/ (git-ignored),
# and verifies each against a pinned SHA-256.
#
# Beat This! (Foscarin, Schlüter and Widmer, ISMIR 2024; MIT), as ONNX files
# converted by beat-this-rs (MIT). The mel front end and the small model are
# pinned to one beat-this-rs commit; the full model is that project's
# "model-large" release asset.
set -euo pipefail

cd "$(dirname "$0")/../.."
mkdir -p models

COMMIT="1ae768e78f1ad83b0ed3886241dc29ffde853c40"
RAW="https://raw.githubusercontent.com/danigb/beat-this-rs/${COMMIT}/models"
RELEASE="https://github.com/danigb/beat-this-rs/releases/download/model-large"

fetch() {
    local name="$1" url="$2" sha="$3"
    if [ -f "models/${name}" ] && echo "${sha}  models/${name}" | shasum -a 256 -c --status; then
        echo "models/${name}: present"
        return
    fi
    curl -fL --retry 3 -o "models/${name}.part" "${url}"
    if ! echo "${sha}  models/${name}.part" | shasum -a 256 -c --status; then
        rm -f "models/${name}.part"
        echo "models/${name}: checksum mismatch, removed" >&2
        exit 1
    fi
    mv "models/${name}.part" "models/${name}"
    echo "models/${name}: downloaded and verified"
}

fetch mel_spectrogram.onnx "${RAW}/mel_spectrogram.onnx" fdd59e65c515331308e4c8841edf99972deca646bdf6197744c2a5b7755e3de9
fetch beat_this_small.onnx "${RAW}/beat_this_small.onnx" a5f8d39d989f31859454ba27afe61c5317ca95e4d9373e6853e5361b8937172f
if [ "${1:-}" = "--full" ]; then
    fetch beat_this.onnx "${RELEASE}/beat_this.onnx" 5f810debe53459b559127fb55bbad40035bb47cc567b20e501670f968c770f02
fi
