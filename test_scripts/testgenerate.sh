#!/usr/bin/env bash
set -euo pipefail

http POST :8000/api/chat/testgenerate | jq --raw-output .id | xargs -I{} http GET :8000/api/chat/{}
