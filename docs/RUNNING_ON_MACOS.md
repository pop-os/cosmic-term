# Executando Cosmic Term no macOS

Este guia explica como compilar e executar o Cosmic Term no macOS (Apple Silicon/Intel), contornando problemas conhecidos de linkagem.

## 1. Pré-requisitos
Algumas dependências do Linux ainda são necessárias para a compilação, mesmo que não sejam usadas ativamente. Instale-as via Homebrew:

```bash
brew install libxkbcommon
```

## 2. Como Rodar (Desenvolvimento)
Para rodar em modo DEBUG (mais rápido para compilar, recomendado para testes):

É necessário apontar o linker para a biblioteca `libxkbcommon` instalada pelo Homebrew:

```bash
# Para Apple Silicon (M1/M2/M3) e Intel
export RUSTFLAGS="-L $(brew --prefix libxkbcommon)/lib"
cargo run
```

## 3. Como Criar Build Final (Release)
Para criar um binário otimizado:

```bash
export RUSTFLAGS="-L $(brew --prefix libxkbcommon)/lib"
cargo build --release
```

O binário será gerado em: `target/release/cosmic-term`

## Solução de Problemas

### Erro: `ld: library 'xkbcommon' not found`
Se você ver este erro, significa que o linker não encontrou a biblioteca. Certifique-se de que a variável `RUSTFLAGS` está definida conforme o passo 2.

### Erro: `feature: winit`
Se ocorrer erros relacionados a features, certifique-se de rodar apenas `cargo run` (que usa o default `wgpu`) e não `cargo run --features winit` (feature inexistente no pacote raiz).
