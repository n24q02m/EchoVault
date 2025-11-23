# EchoVault Developer Handbook

## 1. Project Vision

**EchoVault** is the universal "Black Box" for your AI conversations. It ensures that no insight, code snippet, or debugging session is ever lost, regardless of which IDE or AI tool you use.

### Core Philosophy
-   **Universal Compatibility**: Works with VS Code, Cursor, JetBrains, and more.
-   **Privacy First**: Your chat history is yours. We extract it locally and store it where you want (Git, Local Drive).
-   **Searchable Knowledge**: We turn raw chat logs into a structured, searchable knowledge base.

### Key Features
1.  **Universal Extraction**:
    -   Auto-detects and extracts chat history from IDE-specific databases (e.g., `state.vscdb`).
    -   Supports VS Code, Cursor, Google Antigravity, and JetBrains AI.
2.  **Format Standardization**:
    -   Converts proprietary JSON/SQLite formats into clean, readable **Markdown**.
    -   Preserves code blocks and formatting.
3.  **Git Synchronization**:
    -   Automatically commits and pushes chat history to a private Git repository.
    -   Acts as a "backup" for your thought process.
4.  **Cloud Search (Premium)**:
    -   Syncs metadata to a PostgreSQL database for advanced semantic search across all your conversations.

## 2. System Architecture

EchoVault operates primarily as a **Local CLI Tool** with an optional **Cloud Layer**.

### Components
-   **CLI Tool (Python)**: The core engine. Runs on the user's machine (Windows/WSL/Linux/Mac).
    -   **Extractors**: Modules for each IDE.
    -   **Exporters**: Modules for Markdown, JSON, etc.
    -   **Sync Engine**: Git integration.
-   **Sidecar (Optional)**: A background daemon for real-time syncing.
-   **Cloud Backend (Optional)**: A FastAPI server for indexing and searching chat history (Commercial feature).

### Tech Stack
-   **Core Language**: **Python** (Typer/Click).
-   **Database**:
    -   **Local**: **SQLite** (for caching extraction state).
    -   **Cloud**: **PostgreSQL** (Neon) for search index.
-   **Frontend (Future)**:
    -   **Tauri** + **Next.js** (TypeScript) for a local viewer app.
-   **AI**:
    -   **Local**: Native LLMs for summarization.
    -   **API**: Gemini/Groq for advanced analysis.

## 3. Development Workflow

### Prerequisites
-   Python 3.10+
-   Git

### Setup
1.  **Clone Repository**:
    ```bash
    git clone https://github.com/n24q02m/EchoVault.git
    cd EchoVault
    ```
2.  **Install Dependencies**:
    ```bash
    pip install -r requirements.txt
    ```
3.  **Configuration**:
    -   Set up `config.yaml` with paths to your IDE databases.

### Running
-   **Extract**: `python -m echovault extract`
-   **Sync**: `python -m echovault sync`

## 4. Business Model

**Hybrid Self-Host Open Source + Cloud Freemium**

-   **Core CLI (Open Source)**:
    -   Free forever.
    -   Extracts and saves to local files/Git.
-   **Pro (Cloud/Self-Hosted)**:
    -   Advanced Semantic Search.
    -   Cross-device synchronization of the *index*.
    -   "Chat with your History" feature.

## 5. Roadmap

-   [ ] **Phase 1: The Extractor**: Support VS Code and Cursor. Export to Markdown.
-   [ ] **Phase 2: The Vault**: Git synchronization and local SQLite index.
-   [ ] **Phase 3: The Viewer**: Simple web/Tauri viewer for the Markdown files.
-   [ ] **Phase 4: The Brain**: Semantic search and AI summarization (Cloud/Local Hybrid).
