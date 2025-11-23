# ECHOVAULT - DEVELOPER HANDBOOK

**Phiên bản:** 1.0.0
**Ngày cập nhật:** 23/11/2025
**Dành cho:** Solo Developer

---

## MỤC LỤC

1. [TỔNG QUAN DỰ ÁN](#1-tổng-quan-dự-án)
2. [VISION VÀ SCOPE](#2-vision-và-scope)
3. [KIẾN TRÚC HỆ THỐNG](#3-kiến-trúc-hệ-thống)
4. [CÔNG NGHỆ STACK](#4-công-nghệ-stack)
5. [QUY TRÌNH PHÁT TRIỂN](#5-quy-trình-phát-triển)
6. [CHUẨN MỰC CODE](#6-chuẩn-mực-code)
7. [LỊCH TRÌNH VÀ MILESTONES](#7-lịch-trình-và-milestones)
8. [DEPLOYMENT](#8-deployment)
9. [TROUBLESHOOTING](#9-troubleshooting)

---

## 1. TỔNG QUAN DỰ ÁN

### 1.1. Giới Thiệu

**EchoVault** là "Hộp đen" (Black Box) cho mọi cuộc hội thoại AI của bạn. Dự án đảm bảo rằng không có insight, đoạn code, hay phiên debugging nào bị mất, bất kể bạn sử dụng IDE hay công cụ AI nào.

### 1.2. Người Dùng Mục Tiêu

-   **Developers**: Sử dụng nhiều IDE (VS Code, Cursor, JetBrains) và muốn lưu trữ lịch sử chat tập trung.
-   **Knowledge Workers**: Muốn biến lịch sử chat thành tài sản tri thức có thể tìm kiếm được.

---

## 2. VISION VÀ SCOPE

### 2.1. Core Philosophy

-   **Universal Compatibility**: Hoạt động với mọi IDE phổ biến.
-   **Privacy First**: Dữ liệu được trích xuất cục bộ và lưu trữ theo cách bạn muốn (Git, Local Drive).
-   **Searchable Knowledge**: Biến log chat thô thành Markdown có cấu trúc.

### 2.2. Key Features

1.  **Universal Extraction**:
    -   Tự động phát hiện và trích xuất từ SQLite databases của IDE (ví dụ: `state.vscdb`).
    -   Hỗ trợ VS Code, Cursor, Google Antigravity, JetBrains AI.
2.  **Format Standardization**:
    -   Chuyển đổi JSON/SQLite proprietary thành **Markdown** sạch.
    -   Giữ nguyên code blocks và formatting.
3.  **Git Synchronization**:
    -   Tự động commit và push lịch sử chat lên private Git repository.
    -   Hoạt động như một cơ chế backup tự động.
4.  **Cloud Search (Premium)**:
    -   Sync metadata lên PostgreSQL để tìm kiếm ngữ nghĩa (Semantic Search).

---

## 3. KIẾN TRÚC HỆ THỐNG

### 3.1. Overview

EchoVault hoạt động chủ yếu như một **Local CLI Tool** với tùy chọn **Cloud Layer**.

### 3.2. Components

-   **CLI Tool (Python)**: Engine chính chạy trên máy người dùng.
    -   **Extractors**: Modules xử lý từng loại IDE.
    -   **Exporters**: Modules xuất ra Markdown/JSON.
    -   **Sync Engine**: Tích hợp Git.
-   **Sidecar (Optional)**: Daemon chạy nền để sync realtime.
-   **Cloud Backend (Optional)**: FastAPI server cho indexing và search.

---

## 4. CÔNG NGHỆ STACK

### 4.1. Core Stack

| Layer | Technology | Purpose |
| :--- | :--- | :--- |
| **Language** | Python | Core Logic (Typer/Click) |
| **Local DB** | SQLite | Caching state |
| **Cloud DB** | PostgreSQL (Neon) | Search Index |
| **Sync** | Git | Version Control |

### 4.2. Future Stack (Viewer)

| Layer | Technology | Purpose |
| :--- | :--- | :--- |
| **Frontend** | Next.js (TS) | Web Viewer |
| **Desktop** | Tauri | Local Viewer |

---

## 5. QUY TRÌNH PHÁT TRIỂN

### 5.1. Setup

1.  **Clone Repository**:
    ```bash
    git clone https://github.com/n24q02m/EchoVault.git
    ```
2.  **Install Dependencies**:
    ```bash
    pip install -r requirements.txt
    ```
3.  **Config**:
    -   Thiết lập `config.yaml` với đường dẫn đến các file database của IDE.

### 5.2. Running

-   **Extract**: `python -m echovault extract`
-   **Sync**: `python -m echovault sync`

---

## 6. CHUẨN MỰC CODE

-   **Language**: Code 100% Tiếng Anh. Docs/Comments Tiếng Việt.
-   **Style**: Tuân thủ PEP 8.
-   **Structure**: Modular design (mỗi IDE là một module riêng biệt).

---

## 7. LỊCH TRÌNH VÀ MILESTONES

-   [ ] **Phase 1: The Extractor**: Hỗ trợ VS Code và Cursor. Export ra Markdown.
-   [ ] **Phase 2: The Vault**: Git synchronization và local SQLite index.
-   [ ] **Phase 3: The Viewer**: Web/Tauri viewer đơn giản cho file Markdown.
-   [ ] **Phase 4: The Brain**: Semantic search và AI summarization.

---

## 8. DEPLOYMENT

-   **CLI**: Publish lên PyPI hoặc build thành binary (PyInstaller).
-   **Backend**: Cloud Run (Docker).

---

## 9. TROUBLESHOOTING

-   **Issue**: Database Locked (IDE đang mở).
    -   **Fix**: Copy file database ra temp folder trước khi đọc.
-   **Issue**: Thay đổi cấu trúc JSON của IDE.
    -   **Fix**: Cập nhật parser module thường xuyên.
