# VTX Link - Edge Media Gateway

> **Tailored for Extreme Environments**: A lightweight HLS forwarding and on-demand streaming solution specifically designed for 32MB/64MB RAM edge devices.

---

## Core Philosophy

As a pivotal component of the `vtx-core` ecosystem, **VTX Link** empowers low-resource edge nodes (e.g., OpenWrt routers, legacy gateways) with robust media orchestration capabilities.

* **Resource Awareness**: Real-time monitoring of system memory (RSS/Available RAM). It proactively rejects new tasks before physical memory exhaustion to maintain OS stability.
* **Hardware Longevity**: Native support for RAMDisk-based HLS segment storage (e.g., `/dev/shm`), eliminating high-frequency Flash write cycles and preserving hardware lifespan.
* **Industrial Resilience**: Built-in Exponential Backoff strategy for crash recovery, ensuring autonomous restoration during source-side failures.

## Features

* **On-Demand Activation**: Spawn FFmpeg processes only upon `.m3u8` request; supports configurable idle timeouts for automatic resource reclamation.
* **Ultra-Lightweight**: Written in Rust with a ~5MB binary footprint; features a zero-dependency embedded Single Page Application (SPA) for management.
* **Deep Integration**: Optimized for use with [VTX FFmpeg Release](https://github.com/Vtxdeo/vtx-ffmpeg-release) tailored binaries.
* **Observability**: Integrated dashboard to monitor real-time uptime, idle duration, and crash history.

## Quick Start

### 1. Requirements
- **OS**: Linux (static musl recommended) or Windows.
- **Dependency**: FFmpeg binary (optimized profiles like `vtx-ffmpeg-stream` are recommended).

### 2. Configuration (`vtx-link.yaml`)
```yaml
server:
  listen: "0.0.0.0:8080"
  ffmpeg_binary: "/usr/local/bin/ffmpeg"
  hls_root: "/dev/shm/vtx-hls" # Highly recommended to point to RAMDisk

streams:
  - name: "lobby_cam"
    source: "rtsp://admin:password@192.168.1.10"
    auto_start: true
    idle_timeout: 30
    output_args:
      - "-c"
      - "copy"
      - "-f"
      - "hls"
      - "{output_dir}/index.m3u8"
    retry:
      max_attempts: 5
      initial_backoff_sec: 2

```

### 3. Execution

```bash
./vtx-link --config vtx-link.yaml

```

## Performance Specs (32MB RAM Device)

| Metric | Performance |
| --- | --- |
| **Idle Memory Footprint** | ~4.5 MB |
| **Single HLS Relayer (Copy Mode)** | +2.0 MB ~ 4.0 MB |
| **Max Concurrent Streams** | 3 - 5 (Depending on FFmpeg buffer settings) |

## Architecture

The project follows a modular domain-driven design:

* **Engine**: Manages process lifecycle, RAMDisk mounting, and directory sanitation.
* **Supervisor**: Orchestrates crash recovery and idle stream recycling.
* **Web**: Provides RESTful APIs and the embedded management SPA.

## License

Licensed under the **Apache License, Version 2.0**. See the [LICENSE](LICENSE) file for details.