import os
import sys
import subprocess
import random
import string
import socket
from pathlib import Path

class AnonPeerInstaller:
    def __init__(self):
        self.install_dir = "/opt/AnonPeer"
        self.service_name = "anonpeer"
        self.db_name = "anonpeer"
        self.db_user = "postgres"
        self.port = 3000
        
    def generate_password(self, length=16):
        """Генерация случайного пароля"""
        chars = string.ascii_letters + string.digits
        return ''.join(random.choice(chars) for _ in range(length))
    
    def get_local_ip(self):
        """Получение локального IP адреса"""
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("8.8.8.8", 80))
            ip = s.getsockname()[0]
            s.close()
            return ip
        except:
            return "127.0.0.1"
    
    def run_command(self, cmd, cwd=None, check=True):
        """Выполнение shell команды"""
        print(f"▶ Выполняю: {cmd}")
        try:
            result = subprocess.run(
                cmd, 
                shell=True, 
                cwd=cwd, 
                check=check,
                capture_output=True,
                text=True
            )
            if result.stdout:
                print(result.stdout)
            return True
        except subprocess.CalledProcessError as e:
            print(f"❌ Ошибка: {e.stderr}")
            return False
    
    def install_dependencies(self):
        """Установка необходимых зависимостей"""
        print("\n📦 Установка зависимостей...")
        
        commands = [
            "sudo apt update",
            "sudo apt install -y postgresql postgresql-contrib",
            "sudo apt install -y curl build-essential",
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
        ]
        
        for cmd in commands:
            if not self.run_command(cmd, check=False):
                print(f"⚠️  Предупреждение: команда не выполнена: {cmd}")
        
        # Добавляем cargo в PATH
        self.run_command("source $HOME/.cargo/env", check=False)
        print("✅ Зависимости установлены")
    
    def setup_postgresql(self, password):
        """Настройка PostgreSQL и создание БД"""
        print(f"\n🗄️  Настройка PostgreSQL (пароль: {password})...")
        
        # Запускаем PostgreSQL
        self.run_command("sudo systemctl start postgresql")
        self.run_command("sudo systemctl enable postgresql")
        
        # Устанавливаем пароль для postgres
        sql_commands = f"""
        ALTER USER {self.db_user} WITH PASSWORD '{password}';
        CREATE DATABASE {self.db_name};
        GRANT ALL PRIVILEGES ON DATABASE {self.db_name} TO {self.db_user};
        """
        
        # Выполняем SQL команды
        for sql in sql_commands.strip().split(';'):
            if sql.strip():
                cmd = f'sudo -u postgres psql -c "{sql.strip()}"'
                self.run_command(cmd, check=False)
        
        print("✅ PostgreSQL настроен")
    
    def create_workspace(self):
        """Создание рабочей директории и клонирование репозиториев"""
        print(f"\n📁 Создание рабочей директории {self.install_dir}...")
        
        # Создаем директорию
        self.run_command(f"sudo mkdir -p {self.install_dir}")
        self.run_command(f"sudo chown {os.getenv('USER')}:{os.getenv('USER')} {self.install_dir}")
        
        # Переходим в директорию
        os.chdir(self.install_dir)
        
        # Клонируем репозитории
        repos = [
            ("https://github.com/AnonPeer/AnonPeer-shared.git", "shared"),
            ("https://github.com/AnonPeer/AnonPeer-server.git", "server")
        ]
        
        for repo_url, target_name in repos:
            temp_name = f"AnonPeer-{target_name}"
            self.run_command(f"git clone {repo_url}")
            self.run_command(f"mv {temp_name} {target_name}")
        
        print("✅ Репозитории клонированы")
    
    def create_cargo_toml(self):
        """Создание Cargo.toml"""
        print("\n📝 Создание Cargo.toml...")
        
        cargo_content = """[workspace]
members = ["shared", "server"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
"""
        
        cargo_path = Path(self.install_dir) / "Cargo.toml"
        with open(cargo_path, 'w') as f:
            f.write(cargo_content)
        
        print("✅ Cargo.toml создан")
    
    def create_env_file(self, password):
        """Создание .env файла"""
        print("\n🔧 Создание .env файла...")
        
        ip = self.get_local_ip()
        
        env_content = f"""DATABASE_URL=postgres://{self.db_user}:{password}@localhost:5432/{self.db_name}
SERVER_DOMAIN={ip}:{self.port}
PORT={self.port}
"""
        
        env_path = Path(self.install_dir) / ".env"
        with open(env_path, 'w') as f:
            f.write(env_content)
        
        print(f"✅ .env создан (IP: {ip}, порт: {self.port})")
    
    def build_project(self):
        """Сборка проекта"""
        print("\n🔨 Сборка проекта (это может занять несколько минут)...")
        
        if not self.run_command("cargo build --release", cwd=self.install_dir, check=False):
            print("❌ Ошибка сборки")
            return False
        
        print("✅ Проект собран")
        return True
    
    def create_systemd_service(self, password):
        """Создание systemd service файла"""
        print("\n⚙️  Создание systemd service...")
        
        service_content = f"""[Unit]
Description=AnonPeer Server
After=network.target postgresql.service

[Service]
Type=simple
User={os.getenv('USER')}
WorkingDirectory={self.install_dir}
Environment="DATABASE_URL=postgres://{self.db_user}:{password}@localhost:5432/{self.db_name}"
Environment="SERVER_DOMAIN={self.get_local_ip()}:{self.port}"
Environment="PORT={self.port}"
ExecStart={self.install_dir}/target/release/server
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
"""
        
        service_path = f"/etc/systemd/system/{self.service_name}.service"
        
        # Создаем временный файл
        temp_path = f"/tmp/{self.service_name}.service"
        with open(temp_path, 'w') as f:
            f.write(service_content)
        
        # Копируем в systemd директорию
        self.run_command(f"sudo cp {temp_path} {service_path}")
        self.run_command("sudo systemctl daemon-reload")
        self.run_command(f"sudo systemctl enable {self.service_name}")
        self.run_command(f"sudo systemctl start {self.service_name}")
        
        print(f"✅ Service {self.service_name} создан и запущен")
    
    def install(self):
        """Полная установка"""
        print("=" * 60)
        print("🚀 Установка AnonPeer Server")
        print("=" * 60)
        
        # Генерируем пароль
        password = self.generate_password()
        print(f"\n🔐 Сгенерирован пароль БД: {password}")
        
        # Выполняем установку
        self.install_dependencies()
        self.setup_postgresql(password)
        self.create_workspace()
        self.create_cargo_toml()
        self.create_env_file(password)
        
        if not self.build_project():
            print("\n❌ Установка прервана из-за ошибки сборки")
            return False
        
        self.create_systemd_service(password)
        
        print("\n" + "=" * 60)
        print("✅ Установка завершена успешно!")
        print("=" * 60)
        print(f"\n📊 Информация:")
        print(f"  • Директория: {self.install_dir}")
        print(f"  • IP: {self.get_local_ip()}")
        print(f"  • Порт: {self.port}")
        print(f"  • БД: {self.db_name}")
        print(f"  • Пароль БД: {password}")
        print(f"\n🔧 Управление сервисом:")
        print(f"  • Статус: sudo systemctl status {self.service_name}")
        print(f"  • Перезапуск: sudo systemctl restart {self.service_name}")
        print(f"  • Логи: sudo journalctl -u {self.service_name} -f")
        print("=" * 60)
        
        return True
    
    def uninstall(self):
        """Удаление"""
        print("\n🗑️  Удаление AnonPeer...")
        
        self.run_command(f"sudo systemctl stop {self.service_name}", check=False)
        self.run_command(f"sudo systemctl disable {self.service_name}", check=False)
        self.run_command(f"sudo rm /etc/systemd/system/{self.service_name}.service", check=False)
        self.run_command("sudo systemctl daemon-reload", check=False)
        
        if input(f"Удалить директорию {self.install_dir}? (y/n): ").lower() == 'y':
            self.run_command(f"sudo rm -rf {self.install_dir}", check=False)
        
        if input("Удалить базу данных? (y/n): ").lower() == 'y':
            self.run_command(f"sudo -u postgres psql -c 'DROP DATABASE {self.db_name}'", check=False)
        
        print("✅ Удаление завершено")
    
    def status(self):
        """Проверка статуса"""
        print("\n📊 Статус сервиса:")
        self.run_command(f"sudo systemctl status {self.service_name}", check=False)
        
        print("\n📊 Последние логи:")
        self.run_command(f"sudo journalctl -u {self.service_name} -n 20 --no-pager", check=False)

def main():
    if os.geteuid() != 0:
        print("❌ Скрипт требует root прав. Запустите с sudo:")
        print(f"   sudo python3 {sys.argv[0]}")
        sys.exit(1)
    
    installer = AnonPeerInstaller()
    
    while True:
        print("\n" + "=" * 60)
        print("🔧 AnonPeer Installer Panel")
        print("=" * 60)
        print("1. Установить AnonPeer")
        print("2. Проверить статус")
        print("3. Перезапустить сервис")
        print("4. Удалить AnonPeer")
        print("5. Выход")
        print("=" * 60)
        
        choice = input("Выберите действие (1-5): ").strip()
        
        if choice == "1":
            if input("Начать установку? (y/n): ").lower() == 'y':
                installer.install()
        elif choice == "2":
            installer.status()
        elif choice == "3":
            installer.run_command(f"sudo systemctl restart {installer.service_name}")
            print("✅ Сервис перезапущен")
        elif choice == "4":
            if input("Вы уверены? (y/n): ").lower() == 'y':
                installer.uninstall()
        elif choice == "5":
            print("👋 До свидания!")
            break
        else:
            print("❌ Неверный выбор")

if __name__ == "__main__":
    main()
