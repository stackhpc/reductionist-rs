# Storage

The Reductionist supports access to both S3 and HTTP object stores. Scripts are provided with the source code to allow running local stores, which is particularly useful for testing.

## Scripts for starting local storage

All of the following scripts launch local storage using either [Minio](https://github.com/minio/minio) for S3 or [nginx](https://nginx.org/) for HTTP/HTTPS.

An easy way to manage containers as an unprivileged user is through [Podman](https://podman.io/).

The `podman-docker` package can be installed on `DEB` and `RPM` based systems. It provides a `docker` like interface which is used by the following scripts.

| Script | Description |
| - | - |
| scripts/minio-start | Starts a local Minio container for user-managed S3 storage |
| scripts/minio-stop | Stops the locally started Minio container |
| scripts/nginx-start | Starts a local nginx container for user-managed HTTP storage |
| scripts/nginx-ssl-start | Starts a local nginx container for user-managed HTTPS storage |
| | using user provided TLS certificates |
| scripts/nginx-stop | Stops the locally started nginx container |
| scripts/storage-start | Starts both local Minio and nginx containers for CI/CD |
| scripts/storage-stop | Stops both locally started Minio and nginx containers |

## Using Minio for local S3 storage

[Minio](https://github.com/minio/minio) provides S3-compatible storage, the following scripts allow an existing directory to be used for the storage of Minio exported objects.

Minio stores objects locally in its own file format so the local directory is best populated via Minio's S3 API, but from a test perspective these scripts are useful for persisting a Minio based object store between container restarts. The contents of the existing directory will not be wiped.

### Starting Minio

Checkout the Reductionist source and start Minio as follows:

```shell
git clone https://github.com/stackhpc/reductionist-rs.git
cd reductionist-rs
./scripts/minio-start </path/to/persistent/storage/directory>
```

The directory `/path/to/persistent/storage/directory` must already exist and either be empty or contain files previously created by Minio.

The script starts a detached Minio container, downloading a Minio image if not already downloaded.

The container runs with name `minio-user-share`.

Minio's S3 API is accessible on port 9000.

Minio's web console is accessible on port 9001.

### Stopping Minio

Stop Minio as follows, the script assumes Minio was started with the above script:

```shell
cd /path/to/reductionist-rs
./scripts/minio-stop
```

## Using nginx for local HTTP / HTTPS storage

[nginx](https://nginx.org/) provides HTTP storage, the following scripts allow an existing directory to be used for the storage of objects.

nginx is simply allowing the underlying filesystem to be accessed via HTTP and/or HTTPS.

This is particularly useful for making existing object filestores accessible to the Reductionist without having to worry about file permissions - if the user running podman can access these files they can be made available to the Reductionist.

The included `nginx.conf` has a few useful features, mainly geared towards Reductionist testing of authenticated and unauthenticated access with and without TLS certificates:

| Address | Description |
| - | - |
| http://localhost:8000 | HTTP access to the exported directory |
| http://localhost:8000/private | Authenticated HTTP access to the exported directory |
| http://localhost:8000/upload | HTTP PUT write access to the exported directory |
| | allowing for example CI/CD to test Reductionist's HTTP object store |
| http://localhost:8000/private/upload | Authenticated HTTP PUT write access to the exported directory |
| | allowing for example CI/CD to test Reductionist's HTTP object store |

An additional `nginx-ssl.conf` and accompanying script allow HTTPS using user provided TLS certificates:

| Address | Description |
| - | - |
| https://localhost:8000 | HTTPS access to the exported directory |
| https://localhost:8000/private | Authenticated HTTPS access to the exported directory |
| https://localhost:8000/upload | HTTPS PUT write access to the exported directory |
| | allowing for example CI/CD to test Reductionist's HTTPS object store |
| https://localhost:8000/private/upload | Authenticated HTTPS PUT write access to the exported directory |
| | allowing for example CI/CD to test Reductionist's HTTPS object store |

With the default configuration it's possible to test both authenticated and unauthenticated access to the same location.

This is not very useful from the perspective of securing existing files we want to make accessible to the Reductionist, see the later section on [Authentication](#authentication)

### Starting nginx without TLS certificates

Checkout the Reductionist source and start nginx as follows:

```shell
git clone https://github.com/stackhpc/reductionist-rs.git
cd reductionist-rs
./scripts/nginx-start </path/to/persistent/storage/directory>
```

The directory `/path/to/persistent/storage/directory` must already exist, anything it contains is accessible via nginx.

The script starts a detached nginx container, downloading an nginx image if not already downloaded.

The container runs with name `nginx-user-share`.

nginx is accessible using HTTP on port 8000.

### Stopping nginx

Stop nginx as follows, the script assumes nginx was started with the above script:

```shell
cd /path/to/reductionist-rs
./scripts/nginx-stop
```

### Starting nginx with TLS certificates

Checkout the Reductionist source as follows:

```shell
git clone https://github.com/stackhpc/reductionist-rs.git
```

Edit the script `/path/to/reductionist-rs/scripts/nginx-ssl-start` in the section:

```shell
# This could do with better configuration,
# but to work around permission issues these certificates will be copied
# to a path that can be made accessible within the nginx container.
server_crt="/etc/ssl/default/chain.crt"
server_key="/etc/ssl/default/server.key"
```

- `server_crt` - The path to the full TLS certificate chain
- `server_key` - The path to the TLS certificate private key

Often systems under `/etc/ssl` utilise symbolic links so the script copies them locally to ensure they can be easily volume mounted by the nginx container.

The certificate files referenced need to be readable by the user and the local copies are excluded from version control.

Once edited start nginx as follows:

```shell
cd /path/to/reductionist-rs
./scripts/nginx-ssl-start </path/to/persistent/storage/directory>
```

To stop nginx use the same stop script previously documented above.

### Authentication

With the default nginx configuration it's possible to test both authenticated and unauthenticated access to the same location, but this is not very useful from the perspective of securing existing files we want to make accessible to the Reductionist.

#### Enabling authentication for all access

To enable a completely secured server:

- for HTTP edit `/path/to/reductionist-rs/scripts/nginx.conf`
- for HTTPS edit `/path/to/reductionist-rs/scripts/nginx-ssl.conf`

Look for and uncomment the two `auth_basic` lines:

```config
# Uncomment to require authentication for all access
auth_basic "Access Restricted";
auth_basic_user_file /etc/nginx/htpasswd;
```

#### Username and password used for authentication

A `htpasswd` file is created by the `nginx-start` and `nginx-ssl-start` scripts, the resulting `htpasswd` file is then volume mounted within the nginx container.

The `htpasswd` file is excluded from version control as credentials committed this way are generally flagged as a security risk.

The default credentials are:

- *username* : **admin**
- *password* : **admin**

To change the credentials:

- edit `/path/to/reductionist-rs/scripts/nginx-start` if using the HTTP server
- edit `/path/to/reductionist-rs/scripts/nginx-ssl-start` if using the HTTPS server

Edit the `htpasswd` line:

```shell
# Setup htpasswd for basic authentication.
# NOTE: Do NOT commit generated htpasswd contents to version control,
#       it will be ignored using the following default location.
# To change the credentials used:
# - replace: admin admin
# - with   : <your desired username> <your desired password>
htpasswd -bc "$origin/htpasswd" admin admin
```

## Scripts used by CI/CD

Two additional scripts are provided that start both Minio and nginx for use in CI/CD.

- `scripts/storage-start` - Starts Minio for S3 and nginx for HTTP storage
- `scripts/storage-stop` - Stops Minio and nginx

The scripts manage containers using names `minio` and `nginx` respectively.

The services will be using the same ports as configured in the other scripts above but unlike those scripts **all storage used by these containers is temporary**.

