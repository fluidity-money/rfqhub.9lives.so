#!/bin/sh

size="$(du "$1" | cut -f1)"

if [ "$size" -gt 60 ]; then
	>& echo "$1 too large"
	exit 1
fi

exit 0
