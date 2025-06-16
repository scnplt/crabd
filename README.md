# crabd

![](./assets/crabd.gif)

## Installation

### With prebuild binary file:

Download the prebuild binary file and make it executable:

```bash
# Replace v0.1.0-beta.3 with the version you want to install
curl -L https://github.com/scnplt/crabd/releases/download/v0.1.0-beta.3/crabd -o crabd && chmod +x crabd
```

Run:

```bash
./crabd
```

###  Or build from source:

```bash
git clone https://github.com/scnplt/crabd.git && cd crabd
cargo build --release --target=x86_64-unknown-linux-musl

# Run
./target/x86_64-unknown-linux-musl/release/crabd
```

## Keymap 

| Key | Description |
|-|-|
| J | Down |
| K | Up |
| Q | Quit/Back |
| T | Show all/only running |
| R | Start/Restart |
| S | Stop |
| X | Kill |
| Del/D | Remove |

## License

Copyright (c) 2025 Sertan Canpolat (@scnplt)

Licensed under the [Apache License](./LICENSE), Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
