#!/bin/bash

declare -a USERS=(
  '{"username":"username0","password":"000000","email":"test0@email.com","bio":"💥 Hero-in-training with explosive ambition to be #0! 💣","full_name":"User test0","image_uri":"https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/02/shigaraki-face.jpg"}'
  '{"username":"username1","password":"000000","email":"test1@email.com","bio":"🥇 Champ-in-training with explosive ambition to be #1! 💣","full_name":"User test1","image_uri":"https://fanboydestroy.com/wp-content/uploads/2019/04/ary-and-the-secret-of-seasons-super-resolution-2019.03.22-11.55.42.73.png"}'
  '{"username":"username2","password":"000000","email":"test2@email.com","bio":"‼️ QA-in-training with explosive ambition to be #2! 💣","full_name":"User test2","image_uri":"https://static0.gamerantimages.com/wordpress/wp-content/uploads/2022/07/Genshin-Impact-Sumeru-region.jpg"}'
  '{"username":"username3","password":"000000","email":"test3@email.com","bio":"👾 BOT-in-training with explosive ambition to be #3! 💣","full_name":"User test3","image_uri":"https://static0.gamerantimages.com/wordpress/wp-content/uploads/2023/10/cocoon-container-creature.jpg"}'
  '{"username":"userrr","password":"password","email":"dynamite@myheroacademia.io","bio":"💥 Hero-in-training with explosive ambition to be #1! 💣","full_name":"Katsuki Bakugo","image_uri":"https://qph.cf2.quoracdn.net/main-qimg-64a32df103bc8fb7b2fc495553a5fc0a-lq"}'
)

echo "⏳ Waiting for backend ..."
MAX_ATTEMPTS=100
ATTEMPT=1
until nc -z $HOST $PORT; do 
    if [ $ATTEMPT -ge $MAX_ATTEMPTS ]; then
        echo "❌ Backend did not start after $MAX_ATTEMPTS attempts."
        exit 1
    fi
    sleep 0.5
    ((ATTEMPT++))
done
echo "✅ Backend is up — starting dev env"

for user in "${USERS[@]}"; do
  echo "Registering user: $(echo $user | jq .username)"
  curl -s -X POST -H "Content-Type: application/json" -d "$user" "$SCHEMA://$HOST:$PORT$API_PATH"
  echo -e 
done
