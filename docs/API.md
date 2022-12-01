# 📄 API Documentation

This file contains the API documentation for the Pocket Relay HTTP server

# 📌 Markers
These markers describe certain information about a specific API or route. When
you see these icons next to a route they have the following meanings

| Icon | Meaning                                                                                                                                  |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 🚧    | Work in progress and could change at any time                                                                                            |
| 🔑    | Requires authentication token from the Token API                                                                                         |
| 🟢    | Stable not expected to make structure or breaking changes without notice                                                                 |
| 🔵    | Partially stable but incomplete expecting more routes / features to be added                                                             |
| 🟠    | Semi Stable / Internal feature. Route is used for internal purposes and may change but changes will be reflected in the internal tooling |
| 🔴    | Unstable could change at any time                                                                                                        |


# Token API 🟢

Some routes are protected and require a Token from the tokens API. See
Create Token for information on creating a token. After generating a token
provide it as the X-Token header on future requests

## Create Token

This route is for creating authentication tokens for use throughout the rest of the API. These tokens will expire after 24 hours and will need to be created again when
that happens.

### Example Request

```http
POST /api/token
```

The request body contains the username and password that are used in the server 
configuration file

```json 
{
    "username": "admin",
    "password": "admin"
}
```

### Success Response

The response contains the "token" field which is the token to use in the X-Token header for the other requests.
The "expiry_time" field is the unix time stamp in seconds of when the token will become invalid.

```json
{
    "token": "Tn1RjdQr8Ftrjp1PtED3XRFfKtfcoI6gSdn4F7gyFmmbfCST8aIdLxDWycdChZAh",
    "expiry_time": 1669344878
}
```

### Error Responses 

| Status Code      | Body                | Meaning                                |
| ---------------- | ------------------- | -------------------------------------- |
| 401 Unauthorized | invalid credentials | The username or password was incorrect |



## Validate Token

This route is for validating existing token to check whether the token is still a valid
token or if a new one needs to be generated.

### Example Request

```http
GET /api/token?token=Tn1RjdQr8Ftrjp1PtED3XRFfKtfcoI6gSdn4F7gyFmmbfCST8aIdLxDWycdChZAh
```

The query value of token is the token to check the validity of 

### Valid Token Response

When the token is valid the "valid" field will be true and the unix time in seconds
when the token expires will be the "expiry_time" field
```json 
{
    "valid": true,
    "expiry_time": 1669345922
}
```

### Invalid Token Response

When a token is invalid the "valid" field is false and the "expiry_time" is null

```json
{
    "valid": false,
    "expiry_time": null
}
```

## Delete Token

This route is for deleting tokens to make them invalid before the expiry time 
is reached. Useful for logging out etc.

### Example Request

```http
DELETE /api/token
```

The request body contains the username and password that are used in the server 
configuration file

```json 
{
    "token": "Tn1RjdQr8Ftrjp1PtED3XRFfKtfcoI6gSdn4F7gyFmmbfCST8aIdLxDWycdChZAh",
}
```

### Success Response

This request will always succeed returning the 200 OK status code so the
result of this endpoint can always be ignored




# Server API 🟠

This API is for retrieving information about the server. This includes the verison and the ports
and server types for each of the sub servers

## Get 

```
GET /api/server
```

Simple get request with no paramaters. This route is used internally by the Client tool to ensure
that a server is actually a Pocket Relay server.

### Response

The "version" field is the server version and the "services" field contains a list of each of
the servers running under Pocket Relay along with their ports and the type of server

```json
{
    "version": "0.1.0",
    "services": [
        {
            "name": "Redirector Server",
            "port": 42127,
            "type": "BlazeSecure"
        },
        {
            "name": "Main Server",
            "port": 14219,
            "type": "Blaze"
        },
        {
            "name": "HTTP Server",
            "port": 80,
            "type": "HTTP"
        }
    ]
}
```


# Players API 🔑🔵

This API is for listing players in the database through paginated results or direcly inspecting
the details for a specific player such as their classes, characters, and galaxy at war data.

> This API is likely to have additional POST routes for updating players, characters, classes, and
> galaxy at war data.

## Get Players

```
GET /api/players?offset=0&count=20
```

The query paramater offset is the page offset and the count is the number of players to include
on each page. Offset 1 & Count = 20 = Skip first 20 row and return next 20 rows.


> Omitting the count query parameter will default to 20 players

### Response

The "players" field contains the list of players within the offset and count. The "more" field determines whether there are more players at the next offset value.

```json
{
    "players": [
        {
            "id": 1,
            "email": "test@test.com",
            "display_name": "test@test.com",
            "origin": false,
            "credits": 1666449040,
            "credits_spent": 1668442722,
            "games_played": 16,
            "seconds_played": 3384,
            "inventory": "01010000030000010...LONG VALUE OMMITTED FROM EXAMPLE",
            "csreward": 0,
            "face_codes": "20;",
            "new_item": "20;4;13 223 584,10 75 131,8 98 95 529 93 517 528,9 79 111 84",
            "completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
            "progress": "22,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_timestamps1": "0,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_timestamps2": "0,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_timestamps3": "0,... LONG LIST OMMITTED FROM EXAMPLE"
        },
        {
            "id": 2,
            "email": "test1@test.com",
            "display_name": "test1@test.com",
            "origin": false,
            "credits": 1666449040,
            "credits_spent": 1668442722,
            "games_played": 16,
            "seconds_played": 3384,
            "inventory": "01010000030000010...LONG VALUE OMMITTED FROM EXAMPLE",
            "csreward": 0,
            "face_codes": "20;",
            "new_item": "20;4;13 223 584,10 75 131,8 98 95 529 93 517 528,9 79 111 84",
            "completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
            "progress": "22,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_timestamps1": "0,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_timestamps2": "0,... LONG LIST OMMITTED FROM EXAMPLE",
            "cs_timestamps3": "0,... LONG LIST OMMITTED FROM EXAMPLE"
        },
    ],
    "more": false
}
```

### Error Responses 

| Status Code               | Body                  | Meaning                                 |
| ------------------------- | --------------------- | --------------------------------------- |
| 500 Internal Server Error | Internal Server Error | Database or other server error occurred |


## Get Specific Player

```
GET /api/players/{PLAYER_ID}
```

Replacing {PLAYER_ID} with the ID of the player this route allows you to get only the player data for a player with
a specific ID. This only includes the basic player data and not the classes or characters

### Response

```json
{
    "id": 1,
    "email": "test@test.com",
    "display_name": "test@test.com",
    "origin": false,
    "credits": 1666449040,
    "credits_spent": 1668442722,
    "games_played": 16,
    "seconds_played": 3384,
    "inventory": "01010000030000010...LONG VALUE OMMITTED FROM EXAMPLE",
    "csreward": 0,
    "face_codes": "20;",
    "new_item": "20;4;13 223 584,10 75 131,8 98 95 529 93 517 528,9 79 111 84",
    "completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
    "progress": "22,... LONG LIST OMMITTED FROM EXAMPLE",
    "cs_completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
    "cs_timestamps1": "0,... LONG LIST OMMITTED FROM EXAMPLE",
    "cs_timestamps2": "0,... LONG LIST OMMITTED FROM EXAMPLE",
    "cs_timestamps3": "0,... LONG LIST OMMITTED FROM EXAMPLE"
}
```

### Error Responses 

| Status Code               | Body                                   | Meaning                                    |
| ------------------------- | -------------------------------------- | ------------------------------------------ |
| 404 Not Found             | Couldn't find any players with that ID | Player with matching ID could not be found |
| 500 Internal Server Error | Internal Server Error                  | Database or other server error occurred    |

## Get Specific Player Full

```
GET /api/players/{PLAYER_ID}/full
```

Replacing {PLAYER_ID} with the ID of the player this route allows you to get only the player data for a player with
a specific ID. This includes all the player data, classes, characters and galaxy at war data.

### Response

```json
{
    "player": {
        "id": 1,
        "email": "test@test.com",
        "display_name": "test@test.com",
        "origin": false,
        "credits": 1666449040,
        "credits_spent": 1668442722,
        "games_played": 16,
        "seconds_played": 3384,
        "inventory": "01010000030000010...LONG VALUE OMMITTED FROM EXAMPLE",
        "csreward": 0,
        "face_codes": "20;",
        "new_item": "20;4;13 223 584,10 75 131,8 98 95 529 93 517 528,9 79 111 84",
        "completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
        "progress": "22,... LONG LIST OMMITTED FROM EXAMPLE",
        "cs_completion": "22,... LONG LIST OMMITTED FROM EXAMPLE",
        "cs_timestamps1": "0,... LONG LIST OMMITTED FROM EXAMPLE",
        "cs_timestamps2": "0,... LONG LIST OMMITTED FROM EXAMPLE",
        "cs_timestamps3": "0,... LONG LIST OMMITTED FROM EXAMPLE"
    },
    "classes": [
        {
            "index": 1,
            "name": "Adept",
            "level": 1,
            "exp": 0.0,
            "promotions": 0
        },
        ... Remaining classes ommitted for documentation
    ],
    "characters": [
        {
            "index": 0,
            "kit_name": "AdeptHumanMale",
            "name": "Test",
            "tint1": 0,
            "tint2": 26,
            "pattern": 0,
            "pattern_color": 47,
            "phong": 45,
            "emissive": 9,
            "skin_tone": 9,
            "seconds_played": 0,
            "timestamp_year": 0,
            "timestamp_month": 0,
            "timestamp_day": 0,
            "timestamp_seconds": 0,
            "powers": "Singularity 179 1.000_Shield 89 1.0000 0 0 0 0 0 0 0...Remaining ommited",
            "hotkeys": "",
            "weapons": "0,25",
            "weapon_mods": "",
            "deployed": true,
            "leveled_up": false
        },
        ... Remaining characters ommitted for documentation
    ],
    "galaxy_at_war": {
        "last_modified": "2022-10-29T15:29:22.515609800",
        "group_a": 5300,
        "group_b": 5300,
        "group_c": 5300,
        "group_d": 5300,
        "group_e": 6000
    }
}
```

### Error Responses 

| Status Code               | Body                                   | Meaning                                    |
| ------------------------- | -------------------------------------- | ------------------------------------------ |
| 404 Not Found             | Couldn't find any players with that ID | Player with matching ID could not be found |
| 500 Internal Server Error | Internal Server Error                  | Database or other server error occurred    |


## Get Specific Player Classes

```
GET /api/players/{PLAYER_ID}/classes
```

Replacing {PLAYER_ID} with the ID of the player this route allows you to get only the player data for a player with
a specific ID. This only includes the classes for the player

### Response

```json
[
    {
        "index": 1,
        "name": "Adept",
        "level": 1,
        "exp": 0.0,
        "promotions": 0
    },
    ... Remaining classes ommitted for documentation
]
```

### Error Responses 

| Status Code               | Body                                   | Meaning                                    |
| ------------------------- | -------------------------------------- | ------------------------------------------ |
| 404 Not Found             | Couldn't find any players with that ID | Player with matching ID could not be found |
| 500 Internal Server Error | Internal Server Error                  | Database or other server error occurred    |


## Get Specific Player Characters

```
GET /api/players/{PLAYER_ID}/characters
```

Replacing {PLAYER_ID} with the ID of the player this route allows you to get only the player data for a player with
a specific ID. This only includes the characters for the player

### Response

```json
[
    {
        "index": 0,
        "kit_name": "AdeptHumanMale",
        "name": "Test",
        "tint1": 0,
        "tint2": 26,
        "pattern": 0,
        "pattern_color": 47,
        "phong": 45,
        "emissive": 9,
        "skin_tone": 9,
        "seconds_played": 0,
        "timestamp_year": 0,
        "timestamp_month": 0,
        "timestamp_day": 0,
        "timestamp_seconds": 0,
        "powers": "Singularity 179 1.000_Shield 89 1.0000 0 0 0 0 0 0 0...Remaining ommited",
        "hotkeys": "",
        "weapons": "0,25",
        "weapon_mods": "",
        "deployed": true,
        "leveled_up": false
    },
    ... Remaining characters ommitted for documentation
]
```

### Error Responses 

| Status Code               | Body                                   | Meaning                                    |
| ------------------------- | -------------------------------------- | ------------------------------------------ |
| 404 Not Found             | Couldn't find any players with that ID | Player with matching ID could not be found |
| 500 Internal Server Error | Internal Server Error                  | Database or other server error occurred    |


## Get Specific Player Galaxy At War

```
GET /api/players/{PLAYER_ID}/galaxy_at_war
```

Replacing {PLAYER_ID} with the ID of the player this route allows you to get only the player data for a player with
a specific ID. This only includes the galaxy at war data for the player

### Response

```json
{
    "last_modified": "2022-10-29T15:29:22.515609800",
    "group_a": 5300,
    "group_b": 5300,
    "group_c": 5300,
    "group_d": 5300,
    "group_e": 6000
}
```

### Error Responses 

| Status Code               | Body                                   | Meaning                                    |
| ------------------------- | -------------------------------------- | ------------------------------------------ |
| 404 Not Found             | Couldn't find any players with that ID | Player with matching ID could not be found |
| 500 Internal Server Error | Internal Server Error                  | Database or other server error occurred    |




# Games API 🔑🔵

The games API is for retrieving the details about the active games and the players that
are in those games.

> This API may be altered to include routes for modifying information 
> about the games.

## Games List
```http
GET /api/games
```

This route allows retrieiving a list of all the currently running games 

### Response

```json

{
    "games": [
        {
            "id": 1,
            "state": "InGame",
            "setting": 287,
            "attributes": {
                "ME3_dlc2300": "required",
                "ME3gameEnemyType": "enemy1",
                "ME3_dlc2700": "required",
                "ME3map": "map2",
                "ME3_dlc3225": "required",
                "ME3privacy": "PUBLIC",
                "ME3gameState": "IN_LOBBY",
                "ME3_dlc3050": "required",
                "ME3_dlc2500": "required",
                "ME3gameDifficulty": "difficulty1"
            },
            "players": [
                {
                    "session_id": 1,
                    "player_id": 1,
                    "display_name": "test@test.com",
                    "net": {
                        "groups": {
                            "internal": {
                                "address": "IP ADDRESS OMITTED",
                                "port": 3659
                            },
                            "external": {
                                "address": "IP ADDRESS OMITTED",
                                "port": 3659
                            }
                        },
                        "qos": {
                            "dbps": 0,
                            "natt": "Strict",
                            "ubps": 0
                        },
                        "hardware_flags": 1,
                        "is_set": true
                    }
                }
            ]
        }
    ]
}

```


## Get Game Specific

```http
GET /api/games/{GAME_ID}
```

This route allows retrieving a specific game based on its game ID. Replace {GAME_ID} with the ID of
the game to retrieve

### Response

```json

{
    "id": 1,
    "state": "InGame",
    "setting": 287,
    "attributes": {
        "ME3_dlc2300": "required",
        "ME3gameEnemyType": "enemy1",
        "ME3_dlc2700": "required",
        "ME3map": "map2",
        "ME3_dlc3225": "required",
        "ME3privacy": "PUBLIC",
        "ME3gameState": "IN_LOBBY",
        "ME3_dlc3050": "required",
        "ME3_dlc2500": "required",
        "ME3gameDifficulty": "difficulty1"
    },
    "players": [
        {
            "session_id": 1,
            "player_id": 1,
            "display_name": "test@test.com",
            "net": {
                "groups": {
                    "internal": {
                        "address": "IP ADDRESS OMITTED",
                        "port": 3659
                    },
                    "external": {
                        "address": "IP ADDRESS OMITTED",
                        "port": 3659
                    }
                },
                "qos": {
                    "dbps": 0,
                    "natt": "Strict",
                    "ubps": 0
                },
                "hardware_flags": 1,
                "is_set": true
            }
        }
    ]
}

```

### Error Responses 

| Status Code   | Body                        | Meaning                                  |
| ------------- | --------------------------- | ---------------------------------------- |
| 404 Not Found | Game with that ID not found | Game with matching ID could not be found |


# Leaderboard API 🟢

## N7 Rating Leaderboard

```http
GET /api/leaderboard/n7?count=20&offset=0
```

This route allows you to retrieve the N7 ratings leaderboard. The `count` query parameter is the
number of leaderboard entries to retrieve; Omitting this will default to 20. The `offset` query
parameter is the number of entries to skip from the top. There is another optional query parameter
`player` which when specified will only respond with that entry instead of multiple

### List Response

```json
[
    {
        "player_id": 3,
        "player_name": "Jacobtread",
        "rank": 1,
        "value": 45980
    },
    {
        "player_id": 1,
        "player_name": "test@test.com",
        "rank": 2,
        "value": 61
    },
    {
        "player_id": 4,
        "player_name": "test1@test.com",
        "rank": 3,
        "value": 1
    },
    {
        "player_id": 5,
        "player_name": "test2@test.com",
        "rank": 4,
        "value": 1
    },
    {
        "player_id": 6,
        "player_name": "test3@test.com",
        "rank": 5,
        "value": 1
    }
]

```

### Single Player Response

```json
{
    "player_id": 3,
    "player_name": "Jacobtread",
    "rank": 1,
    "value": 45980
}
```

### Error Responses 

| Status Code               | Body                  | Meaning                                                                                                                      |
| ------------------------- | --------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| 404 Not Found             | Player not found      | The specific player you queried for could not be found its possible the leaderboard hasnt updated to include this player yet |
| 500 Internal Server Error | Server Error Occurred | An error occurred on the server likely a failure when updating the leaderboard                                               |

## Challenge Points Leaderboard

```http
GET /api/leaderboard/cp?count=20&offset=0
```

This route allows you to retrieve the N7 ratings leaderboard. The `count` query parameter is the
number of leaderboard entries to retrieve; Omitting this will default to 20. The `offset` query
parameter is the number of entries to skip from the top. There is another optional query parameter
`player` which when specified will only respond with that entry instead of multiple

### List Response

```json
[
    {
        "player_id": 3,
        "player_name": "Jacobtread",
        "rank": 1,
        "value": 45980
    },
    {
        "player_id": 1,
        "player_name": "test@test.com",
        "rank": 2,
        "value": 61
    },
    {
        "player_id": 4,
        "player_name": "test1@test.com",
        "rank": 3,
        "value": 1
    },
    {
        "player_id": 5,
        "player_name": "test2@test.com",
        "rank": 4,
        "value": 1
    },
    {
        "player_id": 6,
        "player_name": "test3@test.com",
        "rank": 5,
        "value": 1
    }
]

```

### Single Player Response

```json
{
    "player_id": 3,
    "player_name": "Jacobtread",
    "rank": 1,
    "value": 45980
}
```

### Error Responses 

| Status Code               | Body                  | Meaning                                                                                                                      |
| ------------------------- | --------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| 404 Not Found             | Player not found      | The specific player you queried for could not be found its possible the leaderboard hasnt updated to include this player yet |
| 500 Internal Server Error | Server Error Occurred | An error occurred on the server likely a failure when updating the leaderboard                                               |

