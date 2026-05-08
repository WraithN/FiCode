<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh_CN.md">中文</a> |
  <a href="README.fr.md">Français</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.de.md">Deutsch</a>
</p>

# fi-code

Un CLI Agent AI Coding terminal construit en Rust, interagissant avec les utilisateurs via REPL ou TUI. Il prend en charge les conversations multi-tours, les appels d'outils, la persistance des sessions et les extensions de protocole MCP.

## Fonctionnalités

- **🤖 Prise en charge multi-modèles** : Interfaces unifiées compatibles OpenAI et Anthropic avec réponses SSE en streaming
- **🔧 Appels d'outils** : 6 outils intégrés (`bash`, `read`, `write`, `edit`, `web_fetch`, `grep`). L'Agent peut s'exécuter automatiquement en fonction des réponses du modèle et retourner les résultats
- **💬 Persistance des sessions** : Les sessions sont écrites de manière incrémentielle sur le disque local au format JSON Lines, permettant la reprise après interruption
- **🖥️ Mode double interaction** : Interaction REPL traditionnelle et interface TUI plein terminal basée sur `ratatui`
- **🛡️ Validation des permissions** : Niveaux de risque pour les opérations à haut risque comme Bash (Allow / Ask / Deny), interception de `sudo`, `rm -rf` et attaques par injection courantes
- **⚙️ Configuration flexible** : Prend en charge `~/.config/fi-code/config.json` ou `config.jsonc`, avec commentaires, espaces réservés pour variables d'environnement et rechargement à chaud
- **🔗 Support MCP** : Protocole Model Context Protocol implémenté, prenant en charge la gestion multi-serveurs (transport stdio/HTTP)
- **📦 Système de Skills** : Mécanisme d'enregistrement et de chargement de Skills extensible

## Démarrage rapide

### Prérequis

- [Rust](https://rustup.rs/) 1.70+ (dernière version stable recommandée)
- Clé API du fournisseur AI correspondant

### Installation

```bash
# Cloner le dépôt
git clone <repository-url>
cd fi-code

# Compiler
cargo build --release

# Exécuter (mode développement)
cargo run -- --help
```

### Configuration

#### Méthode 1 : Variables d'environnement (Priorité maximale)

**Compatible OpenAI :**
```bash
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_MODEL_NAME=gpt-4o
```

**Anthropic :**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export ANTHROPIC_BASE_URL=https://api.anthropic.com
export ANTHROPIC_MODEL_NAME=claude-3-7-sonnet-20250219
```

#### Méthode 2 : Fichier de configuration

Chemins des fichiers de configuration (recherchés par ordre de priorité) :
- Linux/macOS : `~/.config/fi-code/config.jsonc` ou `~/.config/fi-code/config.json`

Exemple :
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

Prend en charge les commentaires `//` et `/* */`. `apiKey` prend en charge la syntaxe de l'espace réservé `{env:VAR_NAME}`.

### Utilisation

```bash
# Mode REPL interactif
cargo run -- -i

# Mode interface TUI plein terminal
cargo run -- --tui

# Exécuter une commande unique
cargo run -- -c "Écris-moi un Hello World en Rust"

# Afficher les modèles configurés
cargo run -- --models

# Afficher la liste des sessions
cargo run -- -s

# Spécifier le répertoire de travail
cargo run -- -i -w /path/to/project
```

## Structure du projet

```
src/
├── main.rs                 # Point d'entrée du programme
├── agent/                  # Boucle principale Agent et gestion des prompts
├── provider/               # Intégration des modèles (OpenAI / Anthropic)
├── session/                # Gestion des sessions et des messages
├── tools/                  # Registre et implémentation des outils
├── config/                 # Chargement de la configuration et rechargement à chaud
├── permission/             # Classification des risques des permissions
├── tui/                    # Interface utilisateur terminal (ratatui)
├── mcp/                    # Support du protocole MCP
├── skills/                 # Système de Skills
├── commands/               # Commandes slash
└── utils/                  # Utilitaires communs
```

## Outils intégrés

| Outil | Description | Niveau de risque |
|-------|-------------|------------------|
| `bash` | Exécuter des commandes shell | Ask (Commandes dangereuses Deny) |
| `read` / `read_file` | Lire le contenu d'un fichier | Allow |
| `write` | Écrire dans un fichier | Ask |
| `edit` | Éditer un fichier | Ask |
| `web_fetch` | Récupérer une page web et la convertir en Markdown | Ask |
| `grep` | Rechercher dans le contenu des fichiers par regex | Allow |

## Mécanismes de sécurité

- **Protection contre l'évasion de chemin** : Toutes les opérations de fichiers passent par des vérifications `safe_path` pour s'assurer qu'elles ne dépassent pas le répertoire de travail
- **Bac à sable Bash** : Efface les variables d'environnement, ne conserve que les variables minimales nécessaires, délai d'attente de 120 secondes
- **Classification des permissions** : Deny (rejette directement les commandes dangereuses), Ask (confirmation interactive), Allow (les opérations en lecture seule passent directement)
- **Troncature des sorties** : Le contenu retourné par les outils est limité à 50 000 caractères pour éviter de saturer le contexte

## Raccourcis TUI

En mode TUI, les raccourcis suivants sont disponibles :

| Raccourci | Fonction |
|-----------|----------|
| `Tab` / `Shift+Tab` | Changer de zone de focus |
| `Ctrl+C` | Arrêter la génération / quitter le programme |
| `Ctrl+B` | Ouvrir/fermer le tiroir de fichiers à gauche |
| `Ctrl+H` | Ouvrir/fermer le tiroir de l'historique des sessions à droite |
| `Ctrl+M` | Ouvrir la liste déroulante de sélection du modèle |
| `Ctrl+T` | Changer de thème |
| `Ctrl+N` | Nouvelle session |
| `Enter` | Envoyer un message |
| `Shift+Enter` | Nouvelle ligne dans la zone de saisie |
| `Esc` | Fermer le tiroir/liste déroulante/retour à la zone principale |
| `Ctrl+Up` / `PageUp` | Faire défiler la zone de chat vers le haut |
| `Ctrl+Down` / `PageDown` | Faire défiler la zone de chat vers le bas |

## Développement

```bash
# Exécuter les tests
cargo test

# Formater le code
cargo fmt

# Vérification statique Clippy
cargo clippy
```

## Stack technique

| Dépendance | Usage |
|------------|-------|
| `tokio` | Runtime asynchrone |
| `reqwest` | Client HTTP, requêtes SSE en streaming |
| `serde` / `serde_json` | Sérialisation et désérialisation |
| `anyhow` | Gestion des erreurs |
| `rustyline` | Lecture de lignes terminal et historique |
| `ratatui` / `crossterm` | Rendu TUI et événements terminal |
| `colored` | Sortie colorée dans le terminal |
| `clap` | Analyse des arguments en ligne de commande |
| `notify` | Rechargement à chaud du fichier de configuration |
| `regex` | Correspondance par regex |

## Stockage des sessions

Les données de session sont enregistrées au format `.jsonl` dans le répertoire de configuration de la plateforme :
- **Linux** : `~/.config/fi-code/sessions/`
- **macOS** : `~/Library/Application Support/fi-code/sessions/`
- **Windows** : `%APPDATA%\fi-code\sessions\`

## Licence

Ce projet est open-source sous la [Licence MIT](./LICENSE).

Copyright (c) 2025 fi-code contributors.

---

> **Note** : Ce projet est en phase de développement précoce. Les API et les formats de configuration peuvent changer.
