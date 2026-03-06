#!/bin/sh

text='{"type": "service_account",
       ...
      }'
encoded=$(printf '%s' "$text" | base64)

printf '%s\n' "$encoded"
