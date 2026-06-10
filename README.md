# AnonPeer-server
## 📋 Описание

AnonPeer-server — это высокопроизводительный сервер анонимного мессенджера AnonPeer, написанный на языке Rust. Обеспечивает маршрутизацию зашифрованных сообщений между клиентами, управление сессиями пользователей и поддержку федерации — децентрализованного взаимодействия между независимыми серверами.

## ✨ Возможности

- 🚀 Высокопроизводительный асинхронный WebSocket-сервер на базе Axum
- 🗄️ Надёжное хранение данных в PostgreSQL через sqlx
- 🔐 Управление криптографическими ключами пользователей (Ed25519/X25519)
- 🌐 Федерация — обмен сообщениями и ключами между разными серверами
- 🔑 Аутентификация через Argon2 + поддержка сессий с автоматическим переподключением
- 🔍 Поиск пользователей по префиксу юзернейма (включая федеративный поиск)
- 🧵 Полностью асинхронная архитектура на Tokio
- 📊 Структурированное логирование через tracing
- 🛡️ Защита от несанкционированного доступа к приватным данным

## Структура проекта
```
├── Cargo.toml
├── .env.example
├── LICENSE
├── README.md
└── src
    ├── main.rs     
    ├── db.rs      
    ├── auth.rs    
    └── router.rs  
```
## 🚀 Установка

### Требования

- Инструментарий Rust (версия 1.70 или новее)
- Менеджер пакетов Cargo
- PostgreSQL 12+
- Системные инструменты сборки (gcc, make и т.д.)

### Автоматическая установка
```bash
curl -O https://github.com/AnonPeer/AnonPeer-server.git/pamel.py
python3 panel.py
```

### Ручная установка и настрйока
установка необходимых инстурментов
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y postgresql postgresql-contrib build-essential pkg-config libssl-dev

# Установка Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

скачивание репозиториев
```bash
git clone https://github.com/AnonPeer/AnonPeer-client.git
git clone https://github.com/AnonPeer/AnonPeer-shared.git
```

перименование и сборка
```bash
mv AnonPeer-client client
mv AnonPeer-shared shared
cargo build --release
```

создание базы данных
```bash
sudo -u postgres psql
CREATE USER anonuser WITH PASSWORD 'YOUR_PASSWORD';
CREATE DATABASE anonpeer OWNER anonuser;
GRANT ALL PRIVILEGES ON DATABASE anonpeer TO anonuser;
\q
```

настройка .env
```bash
nano /path/to/server/.env

# Подключение к PostgreSQL
DATABASE_URL=postgres://anonuser:your_secure_password@localhost:5432/anonpeer

# Домен/адрес текущего сервера (используется для федерации)
SERVER_DOMAIN=127.0.0.1:3000

# Порт, на котором слушает сервер
PORT=3000

# Список серверов федерации через запятую (для межсерверного поиска)
FEDERATION_PEERS=other-server.com:3000,friend-server.net:3000
```

создание файла systemd
```bash
nano /etc/systemd/system/anonpeer.service
--------------
[Unit]
Description=AnonPeer Server
After=network.target postgresql.service

[Service]
Type=simple
User=anonpeer
WorkingDirectory=/opt/AnonPeer
EnvironmentFile=/opt/AnonPeer/.env
ExecStart=/opt/AnonPeer/target/release/server
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
--------------

sudo systemctl daemon-reload
sudo systemctl enable anonpeer
sudo systemctl start anonpeer
sudo systemctl status anonpeer
```

