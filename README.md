# ser2tcp

Convert a serial com to tcp.

## Simulate virtual serial

```bash
socat -d -d -v pty,rawer,link=$(pwd)/virtual-tty EXEC:$(pwd)/serial-simulator.sh,pty,rawer
```