# Mudanças para Portabilidade macOS

Este documento lista todas as modificações realizadas para portar o `cosmic-term` para macOS.

## Sumário

| Mudança | Arquivo | Motivo |
|---------|---------|--------|
| Features padrão | `Cargo.toml` | macOS não suporta Wayland/D-Bus |
| Daemonização | `src/main.rs` | fork() não é idiomático no macOS |
| Password Manager | `Cargo.toml` | secret-service usa D-Bus (N/A no macOS) |

---

## 1. Features Padrão (`Cargo.toml`)

**Antes:**
```toml
default = ["dbus-config", "wgpu", "wayland", "password_manager"]
```

**Depois:**
```toml
default = ["wgpu"]
```

### Motivo
- `wayland`: macOS usa Cocoa/Metal, não Wayland
- `dbus-config`: D-Bus não existe no macOS
- `password_manager`: Depende de `secret-service` (D-Bus)

### Build por Plataforma

**macOS:**
```bash
cargo build --release
```

**Linux:**
```bash
cargo build --release --features "wayland,dbus-config,password_manager"
```

---

## 2. Daemonização (`src/main.rs`)

**Antes:**
```rust
#[cfg(all(unix, not(target_os = "redox")))]
```

**Depois:**
```rust
#[cfg(all(unix, not(target_os = "redox"), not(target_os = "macos")))]
```

### Motivo
Aplicações macOS não devem fazer `fork()` para se tornarem daemons. O macOS usa `launchd` para serviços em background, e aplicações GUI devem rodar normalmente.

---

## 3. Password Manager (Desabilitado no macOS)

### Motivo
O módulo `password_manager` usa a crate `secret-service`, que se comunica com o daemon de senhas via D-Bus. Como D-Bus não existe no macOS, a feature está desabilitada.

### Alternativa Futura
Para suportar gerenciamento de senhas no macOS, seria necessário:
1. Usar a crate `security-framework` para acessar o macOS Keychain
2. Criar abstração condicional `#[cfg(target_os = "macos")]`

---

## Limitações Conhecidas

| Feature | Status no macOS |
|---------|-----------------|
| Terminal básico | ✅ Funciona |
| Abas e splits | ✅ Funciona |
| Temas de cores | ✅ Funciona |
| Password Manager | ❌ Não disponível |
| Integração D-Bus | ❌ N/A |
| Decorações de janela | ⚠️ Client-side (não nativo) |
