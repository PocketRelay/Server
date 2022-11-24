# 🔧 Configuration

You can configure certain components and settings for the server using Environment Variables. These can either be set as system environment variables or stored in a 
.env file in the same directory or a parent directory to the Pocket Relay executable.

The list of environment variables, their default values, and descriptions are listed below. There is also a default environment variables file named .env.example stored 
in the root of this repository. 

> Any environment variables that you don't provide a value for or provide an invalid
> value for will automatically use the default values. In some cases the server startup
> will be cancelled for an invalid value (I.e. invalid database details)

Items in this list are formatted as the following

```
ENV     : THE_ENV_VARIABLE
TYPE    : The type of value
DEFAULT : The default value
```

The types of environment variables are listed below

| Name    | Examples              | Description                               |
| ------- | --------------------- | ----------------------------------------- |
| TEXT    | Hello world, test.com | Any normal UTF-8 encoded text             |
| BOOLEAN | true, false           | true or false values (Case sensitive)     |
| PORT    | 80,3360,3210          | Any number between 1 and 65536            |
| DECIMAL | 0.5, 1.0, 20          | Any number with an optional decimal place |

# Core Variables

## External Host *(PR_EXT_HOST)*

```
ENV     : PR_EXT_HOST
TYPE    : TEXT
DEFAULT : gosredirector.ea.com
```

This environment variable represents the external address of the server. If you do not
have your own domain that the server is behind then leave this value as is. This default
value is automatically the correct address as its set in the client application.
