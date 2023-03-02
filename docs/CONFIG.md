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


# Server Ports

This section contains the enviroment variables for configuring what ports each
server will use.

## Redirector Port
```
ENV     : PR_REDIRECTOR_PORT
TYPE    : PORT
DEFAULT : 42127
```
> 📌 **WARNING** 📌  This port should not be changed unless the server is being run 
> inside a Docker container or some other solution where the port 42127 is being
> publicly connected to otherwise the Mass Effect 3 client wont be able to connect

This is the port used by the *Redirector* server which is what the Mass Effect 3 client
uses to get the port and address information for the Main server

## Main Port
```
ENV     : PR_MAIN_PORT
TYPE    : PORT
DEFAULT : 14219
```

This is the port used by the *Main* server you can set this port to any port you would like as its port is served through the redirector. This server is responsible for handling
the majority of game logic

## HTTP Port
```
ENV     : PR_HTTP_PORT
TYPE    : PORT
DEFAULT : 80
```

This is the port of the *HTTP* server this is used for things like Galaxy At War, Store Images, Banner Images, and the server API. Using port 80 will make it easiest to use for
accessing through the API and means the clients wont need to specify the port in the 
connection URL

## Telemetry Port
```
ENV     : PR_HTTP_PORT
TYPE    : PORT
DEFAULT : 9988
```

This is the port of the *Telemetry* server this server is not a required portion of the game but it log telemetry recieved from the client if debug logging is enabled

## QOS Port
```
ENV     : PR_HTTP_PORT
TYPE    : PORT
DEFAULT : 17499
```

This is the port of the *Quality Of Service* server this is used to obtain your public IP address so that other players outside of your network can make a connection to you

# Dashboard

Configuration for the dashboard

## Super Admin

```
ENV  : PR_SUPER_ADMIN_EMAIL
TYPE : TEXT
```

In order to grant other users admin access and to have any admin access on the dashboard you must set the email address of a user to become the super user

Super user access will be granted whenever the server starts up and checks if the access is missing and updates accordingly


# Menu 

This section contains configuration for the Main menu

## Menu Message
```
ENV     : PR_MENU_MESSAGE
TYPE    : TEXT
DEFAULT : <font color='#B2B2B2'>Pocket Relay</font> - <font color='#FFFF66'>Logged as: {n}</font>
```

This is the message that is displayed in the scrolling text on the Main Menu in the Mass Effect 3 client. You can use the ``<font color="COLOR"> </font>`` to use hex color codes
and there are variables you can use to insert information about the session

### Variables

| Variable | Description                  |
| -------- | ---------------------------- |
| {v}      | The server verison number    |
| {n}      | The player name              |
| {ip}     | The IP address of the player |

# Galaxy at War 

This section contains configuration for the Galaxy at War system.

## Daily Decay Amount
```
ENV     : PR_GAW_DAILY_DECAY
TYPE    : DECIMAL
DEFAULT : 0.0
```

This value is the percentage decay that the Galaxy At War values will gain for each day
passed.

```
0.5 = -1% each day passed
```

The value of zero means the Galaxy At War percentage will not decay as days pass

## Include Promotions
```
ENV     : PR_GAW_PROMOTIONS
TYPE    : BOOLEAN
DEFAULT : true
```

This variable determines whether player class promotions will be included in the
Galaxy At Rating. true means promotions are included and false means they arent

# Logging

This section contains the configuration for the logging system. Log files are
written in order starting with log.log being the current most recent logs and
then as the file reaches 5mb the old log content will move moved to a new file 
named log-1.log and so on until there are 8 log files then it will go back to 
log-1 and start overwritting the old logs.

## Logging Level
```
ENV     : PR_LOG_LEVEL
TYPE    : TEXT
DEFAULT : info
```

This is the logging level that should be logged. The server will only print logs that are
equal to or above the current log level. Below is a list of the different log levels
in order from lowest to highest.

- debug *Shows all logging including debug messages*
- info *Shows informational logging, warning and error logging*
- warn *Only shows warning and error logs*
- error *Only shows error logs*
- off *Doesn't show any logs at all*

## Logging Directory
```
ENV     : PR_LOGGING_DIR
TYPE    : TEXT
DEFAULT : data/logs
```

This is the path to the folder where server log files should be stored. 


# Retriever

This section contains the configuration for the retriever system which is used to
retrieve information from the official game servers.

## Enabled
```
ENV     : PR_RETRIEVER
TYPE    : BOOLEAN
DEFAULT : true
```
This variable determines whether the retriever system is enabled or not. Setting this
to false will make features such as Origin authentication disabled.

## Origin Fetch
```
ENV     : PR_ORIGIN_FETCH
TYPE    : BOOLEAN
DEFAULT : true
```

This variable determines whether Origin accounts are able to be authenticated. This
authentication uses the retriever system to authenticate on the official servers in
order to get the Origin account information. This requires the retriever system to
be enabled.


## Origin Fetch Data
```
ENV     : PR_ORIGIN_FETCH_DATA
TYPE    : BOOLEAN
DEFAULT : true
```

This variable determines whether all the associated player data for Origin accounts should
be copied over from the official server when first logging into an Origin account. Disable this if you don't want the players having their existing player data from the
official servers copied over.

# Database

This section contains the configuration for the database depending on which server
type you are using.

## SQLite Database
```
ENV     : PR_DATABASE_FILE
TYPE    : TEXT
DEFAULT : data/app.db
```

If you are using the SQLite version (Default) of Pocket Relay then this variable
determines where the database file is saved to. By default this is data/app.db
relative to the server executable.

## MySQL Database
```
ENV     : PR_DATABASE_URL
TYPE    : TEXT
DEFAULT : mysql://username:password@host/database
```

If you are using the MySQL version of Pocket Relay then this variable
determines the connection url for the database.

