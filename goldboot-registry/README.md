## goldboot-registry

HTTP service for storing and serving `goldboot` images.

The server speaks plain HTTP and performs no authentication of its own. TLS
termination and access control are expected to be handled by a reverse proxy
(typically nginx with HTTP Basic Auth).

See the **Registry** section of the [top-level
README](../README.md#registry) for the recommended deployment, an nginx
config snippet, and client usage examples.

### Synopsis

```sh
goldboot-registry start \
  --bind 127.0.0.1:3000 \
  --data-dir /var/lib/goldboot-registry \
  [--max-upload-size <bytes>]
```
