# TURN Server Setup for RDP Remote

This directory contains configuration for the TURN server used in local testing.

## Requirements

- [coturn](https://github.com/coturn/coturn) TURN/STUN server

## Quick Start

1. Install coturn:

```bash
# Ubuntu/Debian
sudo apt install coturn

# macOS (with Homebrew)
brew install coturn
```

2. Start the TURN server:

```bash
turnserver -c turn/turnserver.conf
```

3. Verify the server is running:

```bash
# Test STUN binding
stunclient localhost 3478

# Test TURN allocation (requires credentials)
turnutils uclient -u rdpremote -p rdpremote123 localhost 3478
```

## Configuration

The `turnserver.conf` file contains:

| Setting | Value | Description |
|---------|-------|-------------|
| listening-port | 3478 | Standard STUN/TURN port |
| tls-listening-port | 5349 | TLS port for secure TURN |
| user | rdpremote:rdpremote123 | Static username/password |
| realm | rdpremote.local | TURN realm |
| lt-cred-mech | enabled | Long-term credential mechanism |

## Credentials

| Field | Value |
|-------|-------|
| Username | `rdpremote` |
| Password | `rdpremote123` |

## TURN URL

For WebRTC configuration, use:

```
turn:localhost:3478
```

With credentials:
- username: `rdpremote`
- credential: `rdpremote123`

## Production Notes

For production use:

1. **Use a public TURN server** instead of localhost
2. **Enable TLS** for encrypted TURN traffic
3. **Use dynamic credentials** instead of static passwords
4. **Configure firewall** to allow ports 3478, 5349, and the port range (49152-65535)
5. **Set external-ip** to your public IP address

## Testing with WebRTC

The WebRTC peers (agent and client) are configured to use:

```rust
ice_servers: vec![
    RTCIceServer {
        urls: vec!["turn:localhost:3478".to_string()],
        username: "rdpremote".to_string(),
        credential: "rdpremote123".to_string(),
    },
],
```

## Troubleshooting

- **Connection fails**: Check that the TURN server is running and listening
- **Authentication fails**: Verify username/password match the config
- **ICE candidates not received**: Check firewall rules for UDP ports
