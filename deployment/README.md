# Deployment

This directory contains an Ansible playbook to deploy Reductionist and
supporting services to one or more hosts.

The following OS distributions are supported:

* Ubuntu 20.04-22.04
* CentOS Stream 8-9
* Rocky Linux 8-9

The following services are supported:

* Docker engine
* Step CA Certificate Authority (generates certificates for Reductionist)
* Step CLI (requests and renews certificates)
* Minio object store (optional, for testing)
* Prometheus (monitors Reductionist and HAProxy)
* Jaeger (distributed tracing UI)
* Reductionist
* HAProxy (load balancer for Reductionist)

## Configuration

An example Ansible inventory file is provided in [inventory](inventory) which
defines all groups and maps localhost to them. For a production deployment it
is more typical to deploy to one or more remote hosts.

The following example inventory places HAProxy, Jaeger, Prometheus and Step CA
on `reductionist1`, while Reductionist is deployed on `reductionist1` and
`reductionist2`.

```ini
[haproxy]
reductionist1

[jaeger]
reductionist1

[prometheus]
reductionist1

[reductionist]
reductionist[1:2]

[step:children]
reductionist

[step-ca]
reductionist1

[docker:children]
haproxy
jaeger
minio
prometheus
reductionist
step-ca
```

Some variables are provided to configure the deployment in the
[group_vars](group_vars) directory. Reductionist configuration options may be
specified using environment variables.

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

If you want to run only specific plays in the playbook, the following tags are
supported and may be specified via `--tags <tag1,tag2>`:

* `docker`
* `step-ca`
* `step`
* `minio`
* `prometheus`
* `jaeger`
* `reductionist`
* `haproxy`

## Usage

Once deployed, the Reductionist API is accessible on port 8080 by HAProxy. The
Prometheus UI is accessible on port 9090 on the host running Prometheus. The
Jaeger UI is accessible on port 16686 on the host running Jaeger.
