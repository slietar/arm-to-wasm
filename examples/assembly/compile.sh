#!/bin/bash
set -aeuo pipefail

TEMP_PATH=$(lima mktemp)
OUTPUT_PATH_HOST=$(mktemp)
OUTPUT_PATH_GUEST=$(lima mktemp)
lima aarch64-linux-gnu-as $1 -o $TEMP_PATH && lima aarch64-linux-gnu-ld -o $OUTPUT_PATH_GUEST $TEMP_PATH

lima cat $OUTPUT_PATH_GUEST > $OUTPUT_PATH_HOST
chmod +x $OUTPUT_PATH_HOST
echo $OUTPUT_PATH_HOST

# lima chmod +x $OUTPUT_PATH_GUEST
# lima env $OUTPUT_PATH_GUEST
