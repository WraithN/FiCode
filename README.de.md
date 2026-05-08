<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a>
</p>

# fi-code

Ein in Rust entwickeltes Terminal AI Coding Agent CLI, das über REPL oder TUI mit Benutzern interagiert. Es unterstützt Mehrfachkonversationen, Tool-Aufrufe, Sitzungspersistenz und MCP-Protokollerweiterungen.

## Funktionen

- **🤖 Multi-Modell-Unterstützung**: Vereinheitlichte OpenAI-kompatible und Anthropic-Schnittstellen mit Streaming-SSE-Antworten
- **🔧 Tool-Aufrufe**: 6 integrierte Tools (`bash`, `read`, `write`, `edit`, `web_fetch`, `grep`). Der Agent kann basierend auf Modellantworten automatisch ausführen und Ergebnisse zurückgeben
- **💬 Sitzungspersistenz**: Sitzungen werden inkrementell im JSON Lines-Format auf die lokale Festplatte geschrieben und unterstützen die Wiederaufnahme nach Unterbrechungen
- **🖥️ Dual-Modus-Interaktion**: Traditionelle REPL-Interaktion und Vollterminal-TUI-Oberfläche basierend auf `ratatui`
- **🛡️ Berechtigungsvalidierung**: Risikostufen für Hochrisikooperationen wie Bash (Allow / Ask / Deny), Abfangen von `sudo`, `rm -rf` und gängigen Injection-Angriffen
- **⚙️ Flexible Konfiguration**: Unterstützt `~/.config/fi-code/config.json` oder `config.jsonc`, mit Kommentaren, Umgebungsvariablen-Platzhaltern und Hot-Reload
- **🔗 MCP-Unterstützung**: Model Context Protocol implementiert, unterstützt Multi-Server-Management (stdio/HTTP-Transport)
- **📦 Skills-System**: Erweiterbarer Skill-Registrierungs- und Lademechanismus

## Schnellstart

### Voraussetzungen

- [Rust](https://rustup.rs/) 1.70+ (neueste stabile Version empfohlen)
- Entsprechender AI Provider API Key

### Installation

```bash
# Repository klonen
git clone <repository-url>
cd fi-code

# Kompilieren
cargo build --release

# Ausführen (Entwicklungsmodus)
cargo run -- --help
```

### Konfiguration

#### Methode 1: Umgebungsvariablen (Höchste Priorität)

**OpenAI-kompatibel:**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic:**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

#### Methode 2: Konfigurationsdatei

Konfigurationsdateipfade (in Prioritätsreihenfolge durchsucht):
- Linux/macOS: `~/.config/fi-code/config.jsonc` oder `~/.config/fi-code/config.json`

Beispiel:
```json
{
  "model": "my-model",
  "provider": {
    "openai": {
      "name": "My Provider",
      "options": {
        "apiKey": "{env:MY_API_KEY}",
        "baseURL": "https://api.example.com/v1"
      },
      "models": {
        "my-model": {
          "name": "My Model",
          "limit": { "context": 200000, "output": 65536 }
        }
      }
    }
  }
}
```

Unterstützt `//` und `/* */` Kommentare. `apiKey` unterstützt die `{env:VAR_NAME}` Platzhaltersyntax.

### Verwendung

```bash
# Interaktiver REPL-Modus
cargo run -- -i

# TUI Vollterminal-Modus
cargo run -- --tui

# Einzelnen Befehl ausführen
cargo run -- -c "Schreibe mir ein Rust Hello World"

# Konfigurierte Modelle anzeigen
cargo run -- --models

# Sitzungsliste anzeigen
cargo run -- -s

# Arbeitsverzeichnis angeben
cargo run -- -i -w /path/to/project
```

## Projektstruktur

```
src/
├── main.rs                 # Programm-Einstiegspunkt
├── agent/                  # Agent-Hauptschleife und Prompt-Verwaltung
├── provider/               # Modellintegration (OpenAI / Anthropic)
├── session/                # Sitzungs- und Nachrichtenverwaltung
├── tools/                  # Tool-Registrierung und -Implementierung
├── config/                 # Konfigurationsladen und Hot-Reload
├── permission/             # Berechtigungsrisikostufen
├── tui/                    # Terminal-Benutzeroberfläche (ratatui)
├── mcp/                    # MCP-Protokollunterstützung
├── skills/                 # Skills-System
├── commands/               # Slash-Befehle
└── utils/                  # Gemeinsame Hilfsprogramme
```

## Integrierte Tools

| Tool | Beschreibung | Risikostufe |
|------|-------------|-------------|
| `bash` | Shell-Befehle ausführen | Ask (Gefährliche Befehle Deny) |
| `read` / `read_file` | Dateiinhalt lesen | Allow |
| `write` | In Datei schreiben | Ask |
| `edit` | Datei bearbeiten | Ask |
| `web_fetch` | Webseite abrufen und in Markdown konvertieren | Ask |
| `grep` | Regex-Suche im Dateiinhalt | Allow |

## Sicherheitsmechanismen

- **Pfad-Escape-Schutz**: Alle Dateioperationen durchlaufen `safe_path`-Prüfungen, um sicherzustellen, dass sie das Arbeitsverzeichnis nicht verlassen
- **Bash-Sandbox**: Löscht Umgebungsvariablen, behält nur minimal notwendige Variablen bei, 120-Sekunden-Timeout
- **Berechtigungsstufen**: Deny (gefährliche Befehle direkt ablehnen), Ask (interaktive Bestätigung), Allow (Nur-Lese-Operationen direkt durchlassen)
- **Ausgabe-Kürzung**: Tool-Rückgabeinhalt ist auf 50.000 Zeichen begrenzt, um Kontext-Überlauf zu verhindern

## TUI-Tastenkürzel

Im TUI-Modus sind folgende Tastenkürzel verfügbar:

| Tastenkürzel | Funktion |
|--------------|----------|
| `Tab` / `Shift+Tab` | Fokusbereich wechseln |
| `Ctrl+C` | Generierung stoppen / Programm beenden |
| `Ctrl+B` | Linke Datei-Schublade öffnen/schließen |
| `Ctrl+H` | Rechte Sitzungshistorie-Schublade öffnen/schließen |
| `Ctrl+M` | Modellauswahl-Dropdown öffnen |
| `Ctrl+T` | Thema wechseln |
| `Ctrl+N` | Neue Sitzung |
| `Enter` | Nachricht senden |
| `Shift+Enter` | Neue Zeile im Eingabefeld |
| `Esc` | Schublade/Dropdown schließen/zurück zum Hauptbereich |
| `Ctrl+Up` / `PageUp` | Chat-Bereich nach oben scrollen |
| `Ctrl+Down` / `PageDown` | Chat-Bereich nach unten scrollen |

## Entwicklung

```bash
# Tests ausführen
cargo test

# Code formatieren
cargo fmt

# Clippy statische Prüfung
cargo clippy
```

## Technologie-Stack

| Abhängigkeit | Verwendung |
|--------------|------------|
| `tokio` | Async-Runtime |
| `reqwest` | HTTP-Client, SSE-Streaming-Anfragen |
| `serde` / `serde_json` | Serialisierung und Deserialisierung |
| `anyhow` | Fehlerbehandlung |
| `rustyline` | Terminal-Zeilenlesen und Historie |
| `ratatui` / `crossterm` | TUI-Rendering und Terminal-Ereignisse |
| `colored` | Farbige Terminal-Ausgabe |
| `clap` | Kommandozeilen-Argumentanalyse |
| `notify` | Hot-Reload der Konfigurationsdatei |
| `regex` | Regex-Abgleich |

## Sitzungsspeicher

Sitzungsdaten werden im `.jsonl`-Format im Konfigurationsverzeichnis der Plattform gespeichert:
- **Linux**: `~/.config/fi-code/sessions/`
- **macOS**: `~/Library/Application Support/fi-code/sessions/`
- **Windows**: `%APPDATA%\fi-code\sessions\`

## Lizenz

Dieses Projekt ist unter der [MIT-Lizenz](./LICENSE) Open Source.

Copyright (c) 2025 fi-code contributors.

---

> **Hinweis**: Dieses Projekt befindet sich in einer frühen Entwicklungsphase. APIs und Konfigurationsformate können sich ändern.
