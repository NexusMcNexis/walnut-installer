#!/usr/bin/env bash
# M4: the size budget. Every installer carries its program at the front, so
# the program must stay small. Limits are STAND-IN (in MB), checked on the
# release binaries in DIR:
#   scripts/size_budget.sh DIR
set -euo pipefail
DIR="${1:?usage: size_budget.sh DIR}"
# name, most MB
LIMITS=("walnut-installer 12" "walnut-installer-text 6" "walnut 6" "walnut-builder 18")
fail=0
for entry in "${LIMITS[@]}"; do
	set -- $entry
	[ -f "$DIR/$1" ] || continue
	mb=$(( ($(stat -c %s "$DIR/$1") + 1048575) / 1048576 ))
	if [ "$mb" -gt "$2" ]; then echo "SIZE $1 ${mb} MB > ${2} MB"; fail=1; else echo "SIZE $1 ${mb} MB (budget ${2} MB)"; fi
done
[ "$fail" = 0 ] && echo "SIZE_OK" || { echo "SIZE_OVER"; exit 1; }
