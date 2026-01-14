# Deployment

The [deployment](https://github.com/stackhpc/reductionist-rs/tree/main/deployment) directory in the Reductionist Git repository contains an Ansible playbook to deploy Reductionist and supporting services to one or more hosts.
The Ansible playbook allows for a secure, scale-out deployment of Reductionist, with an HAProxy load balancer proxying requests to any number of Reductionist backend servers.

The following services are supported:

* Podman engine
* Step CA Certificate Authority (generates certificates for Reductionist)
* Step CLI (requests and renews certificates)
* Minio object store (optional, for testing)
* Prometheus (monitors Reductionist and HAProxy)
* Jaeger (distributed tracing UI)
* Reductionist
* HAProxy (load balancer for Reductionist)

## Prerequisites

The existence of correctly configured hosts is assumed by this playbook.

The following host OS distributions have been tested and are supported:

* CentOS Stream 9
* Rocky Linux 9
* Ubuntu 24.04

Currently only a single network is supported.
Several TCP ports should be accessible on this network.
This may require configuration of a firewall on the host (e.g. firewalld, ufw) or security groups in a cloud environment.

* SSH: 22
* Reductionist backend: 8081
* Reductionist frontend: 8080 (HAProxy host only)
* Step CA: 9999 (Step CA host only)
* Minio: 9000 (Minio host only)
* Prometheus: 9090 (Prometheus host only)
* Jaeger: 16686 (Jaeger host only)

The Ansible control host (the host from which you will run `ansible-playbook`) should be able to resolve the hostnames of the hosts.
If names are not provided by DNS, entries may be added to `/etc/hosts` on the Ansible control host.
Issues have been reported when using Ansible with password-protected SSH private keys and SSH agent.

It may be desirable to host the Reductionist API on a different address, such as a hostname or public IP exposed on the host running HAProxy.
This may be configured using the `reductionist_host` variable.

## Configuration

An example Ansible inventory file is provided in [inventory](https://github.com/stackhpc/reductionist-rs/blob/main/deployment/inventory) which defines all groups and maps localhost to them. For a production deployment it is more typical to deploy to one or more remote hosts.

The following example inventory places HAProxy, Jaeger, Prometheus and Step CA on `reductionist1`, while Reductionist is deployed on `reductionist1` and `reductionist2`.

```ini
# Example inventory for deployment to two hosts, reductionist1 and reductionist2.

# HAProxy load balancer.
# Should contain exactly one host.
[haproxy]
reductionist1

# Jaeger distributed tracing UI.
# Should contain at most one host.
[jaeger]
reductionist1

# Prometheus monitoring service.
# Should contain at most one host.
[prometheus]
reductionist1

# Reductionist servers.
# May contain multiple hosts.
[reductionist]
reductionist[1:2]

# Step Certificate Authority (CA).
# Should contain exactly one host.
[step-ca]
reductionist1

# Do not edit.
[step:children]
reductionist

# Do not edit.
[podman:children]
haproxy
jaeger
minio
prometheus
reductionist
step-ca
```

Some variables are provided to configure the deployment in the [group_vars](https://github.com/stackhpc/reductionist-rs/tree/main/deployment/group_vars) directory. Reductionist configuration options may be specified using environment variables specified using `reductionist_env`.

## Ansible control host setup

Whether running Ansible on the same host as the Reductionist server(s) or a separate remote host, some setup is necessary.

Ensure Git and Pip are installed:
```sh
sudo apt -y install git python3-pip # Ubuntu
sudo dnf -y install git python3-pip # CentOS Stream or Rocky Linux
```

Clone the Reductionist source code:
```sh
git clone https://github.com/stackhpc/reductionist-rs
```

Change to the Reductionist source code directory:
```sh
cd reductionist-rs
```

When working with Pip it's generally best to install packages into a virtual environment, to avoid modifying the system packages.
```sh
python3 -m venv venv
source venv/bin/activate
```

## Installation

Install Python dependencies:
```sh
pip install -r deployment/requirements.txt
```

Install Ansible collections:
```sh
ansible-galaxy collection install -r deployment/requirements.yml
```

## Deployment

Podman will be used to run containers under the same user account used for ansible deployment.
To install requisite system packages some tasks will require sudo `privileged` access.

To run the entire playbook as a non-privileged user prompting for a sudo password:
```sh
ansible-playbook -i deployment/inventory deployment/site.yml -K
```

To run specific plays the following tags are supported and may be specified via `--tags <tag1,tag2>`:

* `podman` - runs privileged tasks to install the required system packages
* `step-ca`
* `step` - runs privileged tasks to install the required system packages and Step CA certificate
* `minio`
* `prometheus`
* `jaeger`
* `reductionist`
* `haproxy`

### Minimal deployment of Podman and the Reductionist

Podman is a prerequisite for running the Reductionist.
Podman can run containers as a **non-privileged** user, however this user must have **linger** enabled on their account to allow Podman to continue to run after logging out of the user session.

To enable **linger** support for the non-privileged user:
```sh
sudo loginctl enable-linger <non-privileged user>
```

Alternatively, run the optional `podman` play to install Podman as a **non-privileged** user. The following will prompt for the sudo password to escalate privileges only for package installation and for enabling **linger** for the non-privileged user:
```sh
ansible-playbook -i deployment/inventory deployment/site.yml --tags podman -K
```

Then to run the `reductionist` play, again as the **non-privileged** user:
```sh
ansible-playbook -i deployment/inventory deployment/site.yml --tags reductionist
```

Podman containers require a manual restart after a system reboot.
This requires logging into the host(s) running the Reductionist as the **non-privileged** user to run:
```sh
podman restart reductionist
```

Automatic restart on boot can be enabled via **systemd**, not covered by this documentation.

### Using SSL/TLS certificates with the Reductionist

To enable **https** connections edit `deployment/group_vars/all` before deployment as set:

```
REDUCTIONIST_HTTPS: "true"
```

Note, this is the default.

Create a `certs` directory under the home directory of the non-privileged deployment user, this will be done automatically and the following files will be added if Step is deployed.
If using third party certificates the following files must be added manually using the file names shown:

| Filename | Description |
| -------- | ------- |
| certs/key.pem  | Private key file |
| certs/cert.pem | Certificate file including any intermediates |

Certificates can be added post Reductionist deployment but the Reductionist's container will need to be restarted afterwards.

## Reductionist Configuration

In addition to the `certs` configuration above the file `deployment/group_vars/all` covers the following configuration.

| Ansible Parameter | Description |
| - | - |
| reductionist_build_image | Whether to locally build the Reductionist container |
| reductionist_src_url | Source URL for the Reductionist repository |
| reductionist_src_version | Repository branch to use for local builds |
| reductionist_repo_location | Where to clone the Reductionist repository |
| reductionist_clone_repo | By default the repository cloning overwrites local changes, this disables |
| reductionist_name | Name for Reductionist container |
| reductionist_image | Container URL if downloading and not building |
| reductionist_tag | Container tag |
| reductionist_networks | List of container networks |
| reductionist_env | Configures the Reductionist environment, see table of environment variables below |
| reductionist_remote_certs_path | Path to certificates on the host |
| reductionist_container_certs_path | Path to certificates within the container |
| reductionist_remote_cache_path | Path to cache on host filesystem |
| reductionist_container_cache_path | Path to cache within the container |
| reductionist_volumes | Volumes to map from host to container |
| reductionist_host | Used when deploying HAProxy to test connectivity to backend Reductionist(s) |
| reductionist_cert_not_after | Certificate validity |

The ``reductionist_env`` parameter allows configuration of the environment variables passed to the Reductionist at runtime:

| Environment Variable | Description |
| - | - |
| REDUCTIONIST_HOST | The IP address on which to listen on, default "0.0.0.0" |
| REDUCTIONIST_PORT | Port to listen on |
| REDUCTIONIST_HTTPS | Whether to enable https connections |
| REDUCTIONIST_CERT_FILE | Path to the certificate file used for https |
| REDUCTIONIST_KEY_FILE | Path to the key file used for https |
| REDUCTIONIST_SHUTDOWN_TIMEOUT | Maximum time in seconds to wait for operations to complete after receiving the 'ctrl+c' signal |
| REDUCTIONIST_ENABLE_JAEGER | Whether to enable sending traces to Jaeger |
| REDUCTIONIST_USE_RAYON | Whether to use Rayon for execution of CPU-bound tasks |
| REDUCTIONIST_MEMORY_LIMIT | Memory limit in bytes |
| REDUCTIONIST_S3_CONNECTION_LIMIT | S3 connection limit |
| REDUCTIONIST_THREAD_LIMIT | Thread limit for CPU-bound tasks |
| REDUCTIONIST_USE_CHUNK_CACHE | Whether to enable caching of downloaded data objects to disk |
| REDUCTIONIST_CHUNK_CACHE_PATH | Absolute filesystem path used for the cache. Defaults to container cache path, see Ansible Parameters above |
| REDUCTIONIST_CHUNK_CACHE_AGE | Time in seconds a chunk is kept in the cache |
| REDUCTIONIST_CHUNK_CACHE_PRUNE_INTERVAL | Time in seconds between periodic pruning of the cache |
| REDUCTIONIST_CHUNK_CACHE_SIZE_LIMIT | Maximum cache size, i.e. "100GB" |
| REDUCTIONIST_CHUNK_CACHE_QUEUE_SIZE | Tokio MPSC buffer size used to queue downloaded objects between the asynchronous web engine and the synchronous cache |
| REDUCTIONIST_CHUNK_CACHE_KEY | Overrides the key format used to uniquely identify a cached chunk, see section below |
| REDUCTIONIST_CHUNK_CACHE_BYPASS_AUTH | Allow bypassing of S3 authentication when accessing cached data |


Note, after changing any of the above parameters the Reductionist must be deployed, or redeployed, using the ansible playbook for the change to take effect.
The idempotent nature of ansible necessitates that if redeploying then a running Reductionist container must be removed first.

### Chunk Cache Key

This defines the name of the key which should uniquely identify a downloaded chunk.
The default value is "%source-%bucket-%object-%offset-%size-%auth". All the parameters used here would be used in the API call to download an S3 object and so should uniquely identify an object.
The assumption is made that the object on the S3 data store doesn't change, i.e. replaced using different compression.

* Use insufficient parameters to uniquely identify a chunk and a request may be served with a cached chunk containing the wrong data
* Use too many parameters, unnecessary ones, and we're missing out on cache hits

#### Chunk Cache Key Tokens Available

| Token | Description |
| - | - |
| `%source` | Source URL for S3 data store |
| `%bucket` | S3 bucket |
| `%object` | Object key |
| `%offset` | Offset of data byte range |
| `%size` | Size of data byte range |
| `%dtype` | Data type |
| `%byte_order` | Byte order of data | 
| `%compression` | Type of compression used on data |
| `%auth` | Client credentials |

Where request parameters are optional, so may not be present in all requests, their tokens will always be usable with null values constructing the cache key.

#### Authenticating Cached Chunks

The original request to download data from S3 will be authenticated.
Data cached from this request is likely subject to authentication also, to ensure a different Reductionist client can't read private data via the cache, there are three approaches that can be taken with authentication.

##### No Authentication

Validating that a client is authorised to access a cached chunk will always add an overhead, however small, and it should be noted that the Reductionist doesn't maintain an internal state of authenticated clients vs chunks.
Performance will always be best when authentication is disabled which is achieved with the following configuration:

| Environment Variable | Description | |
| - | - | - |
| REDUCTIONIST_CHUNK_CACHE_KEY | "%source-%bucket-%object-%offset-%size"` | Namely the key should not contain `%auth` |
| REDUCTIONIST_CHUNK_CACHE_BYPASS_AUTH | true | Disable S3 authentication check prior to retrieving cached chunk |

##### Client can only retrieve chunks that they cached

If authentication is required then the fastest option is to store cached chunks per client, with this approch two clients requesting the same chunk will each cache their own chunks independently.

| Environment Variable | Description | |
| - | - | - |
| REDUCTIONIST_CHUNK_CACHE_KEY | "%source-%bucket-%object-%offset-%size-%auth"` | Namely the key should contain `%auth` |
| REDUCTIONIST_CHUNK_CACHE_BYPASS_AUTH | true | Disable S3 authentication check prior to retrieving cached chunk |

The key name, once constructed from parameters, is [MD5](https://en.wikipedia.org/wiki/MD5) encoded so credentials aren't exposed via the chunk cache filesystem.

##### Clients can retrieve any cached chunks to which S3 grants access

If cached chunks are to be shared between clients then the Reductionist can perform a S3 authentication check prior to retrieving the cached chunk. There is an API call to the S3 object store associated with this so server and network latency will factor in, this approach should be benchmarked against the other authentication method documented above to see which best fits.

| Environment Variable | Description | |
| - | - | - |
| REDUCTIONIST_CHUNK_CACHE_KEY | "%source-%bucket-%object-%offset-%size"` | Namely the key should not contain `%auth` |
| REDUCTIONIST_CHUNK_CACHE_BYPASS_AUTH | true | Enable S3 authentication check prior to retrieving cached chunk |

## Usage

Once deployed, the Reductionist API is accessible on port 8080 by HAProxy. The Prometheus UI is accessible on port 9090 on the host running Prometheus. The Jaeger UI is accessible on port 16686 on the host running Jaeger.
