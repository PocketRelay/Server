# API is not currently stable and may change.

This documentation is not yet complete and is expected to change


# Token API

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