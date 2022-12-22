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

# Dynamic Paths

Certain ruotes contain dynamically matched paths such as /api/players/:player_id matched portions of paths start with `:` when you see part of a path starting with `:` you should
replace this part of the URL with a specific value


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

| Status Code      | Body               | Meaning                                |
| ---------------- | ------------------ | -------------------------------------- |
| 401 Unauthorized | InvalidCredentials | The username or password was incorrect |



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
            "type": "Http"
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
The "offset" field contains the current offset page provided by the query and the "count" is the count expected by the query (The count is NOT the number of players returned)

```json
{
    "players": [
        {
            "id": 1,
            "email": "test@test.com",
            "display_name": "test@test.com",
            "origin": false,
        },
        {
            "id": 2,
            "email": "test1@test.com",
            "display_name": "test1@test.com",
            "origin": false,
        },
    ],
    "more": false
}
```

### Error Responses 

| Status Code               | Body        | Meaning                                 |
| ------------------------- | ----------- | --------------------------------------- |
| 500 Internal Server Error | ServerError | Database or other server error occurred |


## Create Player

```
POST /api/players
```
```json
{
    "email": "test12@test.com",
    "display_name": "Test 12",
    "password": "test"
}
```

### Response

```json
{
	"id": 14,
	"email": "test12@test.com",
	"display_name": "Test 12",
	"origin": false,
}
```
### Error Responses 

| Status Code               | Body         | Meaning                                                 |
| ------------------------- | ------------ | ------------------------------------------------------- |
| 400 Bad Request           | EmailTaken   | The provided email address is already in use            |
| 400 Bad Request           | InvalidEmail | The provided email address is not a valid email address |
| 500 Internal Server Error | ServerError  | Database or other server error occurred                 |


## Get Specific Player

```
GET /api/players/:player_id
```

Replacing :player_id with the ID of the player this route allows you to get only the player data for a player with
a specific ID. This only includes the basic player data and not the classes or characters

### Response

```json
{
    "id": 1,
    "email": "test@test.com",
    "display_name": "test@test.com",
    "origin": false,
}
```

### Error Responses 

| Status Code               | Body           | Meaning                                    |
| ------------------------- | -------------- | ------------------------------------------ |
| 404 Not Found             | PlayerNotFound | Player with matching ID could not be found |
| 500 Internal Server Error | ServerError    | Database or other server error occurred    |


## Modify Player

```
PUT /api/players/:player_id
```

```json 
{
    "email": "test@test.com",
    "display_name": "Test 1",
    "origin": false,
    "password": "Some example field
}
```

### Fields

Below is a table of fields that you can include within your JSON
request

| Key          | Optional | Description                                                                 |
| ------------ | -------- | --------------------------------------------------------------------------- |
| email        | Yes      | The new email address for this player (Will give an error if already taken) |
| display_name | Yes      | The new display name for this player                                        |
| origin       | Yes      | Whether this account is an origin account                                   |
| password     | Yes      | A new plaintext password to be hashed for the player                        |

Replacing :player_id with the ID of the player 

### Response

The response is the player structure but with the new values updated

```json
{
    "id": 1,
    "email": "test@test.com",
    "display_name": "Test 1",
    "origin": false,
}
```

| Status Code               | Body           | Meaning                                                 |
| ------------------------- | -------------- | ------------------------------------------------------- |
| 404 Not Found             | PlayerNotFound | Player with matching ID could not be found              |
| 400 Bad Request           | EmailTaken     | The provided email address is already in use            |
| 400 Bad Request           | InvalidEmail   | The provided email address is not a valid email address |
| 500 Internal Server Error | ServerError    | Database or other server error occurred                 |



## Get Specific Player Galaxy At War

```
GET /api/players/:player_id/galaxy_at_war
```
This route retrieves the galaxy at war data for the provided player. If the data has not yet been generated new default data will be generated.


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

| Status Code               | Body           | Meaning                                    |
| ------------------------- | -------------- | ------------------------------------------ |
| 404 Not Found             | PlayerNotFound | Player with matching ID could not be found |
| 500 Internal Server Error | ServerError    | Database or other server error occurred    |




# Games API 🔑🔵

The games API is for retrieving the details about the active games and the players that
are in those games.

> This API may be altered to include routes for modifying information 
> about the games.

## Games List
```http
GET /api/games?offset=0&count=20
```
This route allows you to retrieve a list of games form the server. Responses are paginated

### Query Paramaters

| Key    | Optional | Description                                                                   |
| ------ | -------- | ----------------------------------------------------------------------------- |
| offset | Yes      | Optional offset parameter to offset the current page (start = offset * count) |
| count  | Yes      | Optional count value to change how many games are returned                    |

> The default count value is 20 games to prevent and the maximum count value is 255 to prevent the server from having to serialize massive lists you should use this in a paginated way instead of querying all 255 games


### Response
The "games" field contains a list of games that are running on the server. The "more" field contains whether there are more games at the next offset value which can be used to determine whether a next page is available for pagination
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
    ],
    "more": false,
}

```


## Get Game Specific

```http
GET /api/games/:game_id
```

This route allows retrieving a specific game based on its game ID. Replace :game_id with the ID of
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

| Status Code   | Body         | Meaning                                  |
| ------------- | ------------ | ---------------------------------------- |
| 404 Not Found | GameNotFound | Game with matching ID could not be found |


# Leaderboard API 🟢

API for accessing the leaderboards stored within the server. (Leaderboards are cached internally for 1 hour both for the API and for the in game leaderboard)

## Leaderboard Keys

The following table is the available leaderboard keys that can be used as the :name route path to decide
which leaderboard to obtain entires from

| Name | Description                                                 |
| ---- | ----------------------------------------------------------- |
| n7   | Leaderboard ranked on the N7 Rating of each player          |
| cp   | Leaderboard ranked on Challenge point count  of each player |

These keys are used by both of the leaderboard endpoints

## List Leaderboard

```http
GET /api/leaderboard/:name?count=20&offset=0
```
This route allows you to retrieve a specific leaderboard. Responses are paginated

### Query Paramaters

| Key    | Optional | Description                                                                   |
| ------ | -------- | ----------------------------------------------------------------------------- |
| offset | Yes      | Optional offset parameter to offset the current page (start = offset * count) |
| count  | Yes      | Optional count value to change how many entries are returned                  |

> The default count value is 40 entries to prevent and the maximum count value is 255 to prevent the server from having to serialize massive lists you should use this
> in a paginated way instead of querying all 255 entries

### Response

The "entries" field contains all the leaderboard entries at the current offset which is at most the provided
count. The "more" field contains whether there are more entires at the next offset value which can be used to 
determine whether a next page is available for pagination

```json
{
    "entries": [
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
    ],
    "more": true
}

```

### Error Responses 

| Status Code               | Body               | Meaning                                                                        |
| ------------------------- | ------------------ | ------------------------------------------------------------------------------ |
| 404 Not Found             | UnknownLeaderboard | The leaderboard key you used was not valid                                     |
| 500 Internal Server Error | ServerError        | An error occurred on the server likely a failure when updating the leaderboard |


## Specific player ranking

```http
GET /api/leaderboard/:name/:player_id
```

This route allows you to retrieve the leader board entry of a specific player using the players ID.

### Response

```json
{
    "player_id": 3,
    "player_name": "Jacobtread",
    "rank": 1,
    "value": 45980
}
```

### Error Responses 

| Status Code               | Body               | Meaning                                                                                                                      |
| ------------------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| 404 Not Found             | PlayerNotFound     | The specific player you queried for could not be found its possible the leaderboard hasnt updated to include this player yet |
| 404 Not Found             | UnknownLeaderboard | The leaderboard key you used was not valid                                                                                   |
| 500 Internal Server Error | ServerError        | An error occurred on the server likely a failure when updating the leaderboard                                               |
