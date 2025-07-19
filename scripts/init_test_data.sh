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

declare -A REGISTERED_USERS=()

index=0

echo ""
echo "Create user data..."

for user in "${USERS[@]}"; do
  username=$(echo "$user" | jq -r .username)
  echo "🔄 Registering user: $username"
  
  response=$(curl -s -X POST -H "Content-Type: application/json" -d "$user" "$SCHEMA://$HOST:$PORT$API_PATH")
  
  token=$(echo "$response" | jq -r '.token' 2>/dev/null)
  encode_userid=$(echo "$response" | jq -r '.user.id.tb + "%3A" + .user.id.id.String')
  userid=$(echo "$response" | jq -r '.user.id.tb + ":" + .user.id.id.String')
  
  discussion_id="discussion%3A$(echo "$response" | jq -r '.user.id.id.String')"
  curl -s -X GET "$SCHEMA://$HOST:$PORT/test/api/endow/$encode_userid/100"
post_uris=()  # make sure it's cleared/reset
for i in {1..3}; do
  response=$(curl -s -X POST "$SCHEMA://$HOST:$PORT/api/discussions/$discussion_id/posts" \
    -H "Accept: application/json" \
    -b "jwt=$token" \
    -F "title=Post $i" \
    -F "topic_id=" \
    -F "content=Lorem Ipsum")

  echo "$response"  # debug log

  post_id=$(echo "$response" | jq -r '.id.tb + ":" + .id.id.String')
  [[ -n "$post_id" ]] && post_uris+=("$post_id")

  curl -s -X POST "$SCHEMA://$HOST:$PORT/api/posts/$post_id/replies" \
    -H "Content-Type: application/json" \
    -b "jwt=$token" \
    -d '{"title": "Reply 1", "content": "Lorem Ipsum"}'
done

  post_uris_json=$(printf '%s\n' "${post_uris[@]}" | jq -R . | jq -s .)

  REGISTERED_USERS["$index"]=$(jq -n \
    --arg id "$userid" \
    --arg token "$token" \
    --argjson posts "$post_uris_json" \
    '{id: $id, token: $token, posts: $posts}')

  ((index++))
done
echo ""
echo "Setting up follow relationships..."

token=$(echo "${REGISTERED_USERS[0]}" | jq -r '.token')
id1=$(echo "${REGISTERED_USERS[1]}" | jq -r '.id')

curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id1" \
  -H "Accept: application/json" \
  -b "jwt=$token"

id2=$(echo "${REGISTERED_USERS[2]}" | jq -r '.id')
curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id2" \
  -H "Accept: application/json" \
  -b "jwt=$token"

id3=$(echo "${REGISTERED_USERS[3]}" | jq -r '.id')
curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id3" \
  -H "Accept: application/json" \
  -b "jwt=$token"

token=$(echo "${REGISTERED_USERS[1]}" | jq -r '.token')
id0=$(echo "${REGISTERED_USERS[0]}" | jq -r '.id')

curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id1" \
  -H "Accept: application/json" \
  -b "jwt=$token"

id2=$(echo "${REGISTERED_USERS[2]}" | jq -r '.id')
curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id2" \
  -H "Accept: application/json" \
  -b "jwt=$token"

id3=$(echo "${REGISTERED_USERS[3]}" | jq -r '.id')
curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id3" \
  -H "Accept: application/json" \
  -b "jwt=$token"

token=$(echo "${REGISTERED_USERS[2]}" | jq -r '.token')
id0=$(echo "${REGISTERED_USERS[0]}" | jq -r '.id')

curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id1" \
  -H "Accept: application/json" \
  -b "jwt=$token"

id1=$(echo "${REGISTERED_USERS[1]}" | jq -r '.id')
curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id2" \
  -H "Accept: application/json" \
  -b "jwt=$token"

id3=$(echo "${REGISTERED_USERS[3]}" | jq -r '.id')
curl -s -X POST "$SCHEMA://$HOST:$PORT/api/followers/$id3" \
  -H "Accept: application/json" \
  -b "jwt=$token"

echo ""
echo "Setting up challenge..."


user1_token=$(echo "${REGISTERED_USERS[0]}" | jq -r '.token')
user2_token=$(echo "${REGISTERED_USERS[1]}" | jq -r '.token')
user3_token=$(echo "${REGISTERED_USERS[2]}" | jq -r '.token')
user1_posts_json=$(echo "${REGISTERED_USERS[0]}" | jq -c '.posts')  
user2_posts_json=$(echo "${REGISTERED_USERS[1]}" | jq -c '.posts')  
user3_posts_json=$(echo "${REGISTERED_USERS[2]}" | jq -c '.posts')  
user1_id=$(echo "${REGISTERED_USERS[0]}" | jq -r '.id')
user2_id=$(echo "${REGISTERED_USERS[1]}" | jq -r '.id')
user3_id=$(echo "${REGISTERED_USERS[2]}" | jq -r '.id')

echo "$user1_posts_json" | jq -r '.[]' | while IFS= read -r post_id; do
  curl -s -X POST "$SCHEMA://$HOST:$PORT/api/posts/$post_id/tasks" \
    -H "Content-Type: application/json" \
    -b "jwt=$user1_token" \
    -d "{
      \"participant\": \"$user2_id\",
      \"offer_amount\": 10,
      \"content\": \"Task for user2 from post $post_id\"
    }"

  curl -s -X POST "$SCHEMA://$HOST:$PORT/api/tasks/$post_id/tasks" \
    -H "Content-Type: application/json" \
    -b "jwt=$user1_token" \
    -d "{
      \"participant\": \"$user3_id\",
      \"offer_amount\": 10,
      \"content\": \"Task for user3 from post $post_id\"
    }"
done

echo "$user2_posts_json" | jq -r '.[]' | while IFS= read -r post_id; do
  curl -s -X POST "$SCHEMA://$HOST:$PORT/api/posts/$post_id/tasks" \
    -H "Content-Type: application/json" \
    -b "jwt=$user2_token" \
    -d "{
      \"participant\": \"$user3_id\",
      \"offer_amount\": 10,
      \"content\": \"Task for\"
    }"
done

echo "$user3_posts_json" | jq -r '.[]' | while IFS= read -r post_id; do
  curl -s -X POST "$SCHEMA://$HOST:$PORT/api/posts/$post_id/tasks" \
    -H "Content-Type: application/json" \
    -b "jwt=$user3_token" \
    -d "{
      \"participant\": \"$user1_id\",
      \"offer_amount\": 10,
      \"content\": \"Task for\"
    }"
done

echo ""
echo "🎉 Development environment setup complete!"
