# Deployment

The [deployment](https://github.com/stackhpc/reductionist-rs/tree/main/deployment) directory in the Reductionist Git repository contains an Ansible playbook to deploy Reductionist and supporting services to one or more hosts.
The Ansible playbook allows for a secure, scale-out deployment of Reductionist, with an HAProxy load balancer proxying requests to any number of Reductionist backend servers.

The following services are supported:

* Docker engine
* Step CA Certificate Authority (generates certificates for Reductionist)
* Step CLI (requests and renews certificates)
* Minio object store (optional, for testing)
* Prometheus (monitors Reductionist and HAProxy)
* Jaeger (distributed tracing UI)
* Reductionist
* HAProxy (load balancer for Reductionist)

## Prerequisites

The existence of correctly configured hosts is assumed by this playbook.

The following host OS distributions are supported:

* Ubuntu 20.04-22.04
* CentOS Stream 8-9
* Rocky Linux 8-9

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
[docker:children]
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

Run the playbook:
```sh
ansible-playbook -i deployment/inventory deployment/site.yml
```

If you want to run only specific plays in the playbook, the following tags are supported and may be specified via `--tags <tag1,tag2>`:

* `docker`
* `step-ca`
* `step`
* `minio`
* `prometheus`
* `jaeger`
* `reductionist`
* `haproxy`

## Usage

Once deployed, the Reductionist API is accessible on port 8080 by HAProxy. The Prometheus UI is accessible on port 9090 on the host running Prometheus. The Jaeger UI is accessible on port 16686 on the host running Jaeger.
