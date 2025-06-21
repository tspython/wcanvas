#!/bin/bash

INPUT="${1:-../../data/fonts/Virgil.woff2}"
OUTPUT="${2:-${INPUT%.*}.ttf}"

if [ ! -f "$INPUT" ]; then
    echo "Error: $INPUT not found"
    exit 1
fi

if command -v woff2_decompress >/dev/null 2>&1; then
    woff2_decompress "$INPUT"
    EXPECTED="${INPUT%.*}.ttf"
    if [ -f "$EXPECTED" ] && [ "$EXPECTED" != "$OUTPUT" ]; then
        mv "$EXPECTED" "$OUTPUT"
    fi
elif command -v python3 >/dev/null 2>&1; then
    python3 -c "from fontTools.ttLib import TTFont; TTFont('$INPUT').save('$OUTPUT')" 2>/dev/null || {
        echo "Install: brew install woff2 or pip install fonttools"
        exit 1
    }
else
    echo "Install: brew install woff2 or pip install fonttools"
    exit 1
fi

echo "Converted: $OUTPUT" 