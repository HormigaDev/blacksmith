# Blacksmith

[![Client checks](https://github.com/HormigaDev/blacksmith/actions/workflows/client.yml/badge.svg)](https://github.com/HormigaDev/blacksmith/actions/workflows/client.yml)
[![Rust](https://img.shields.io/badge/Rust-1.97%2B-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![Licença: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)

[English](../README.md) | [Español](./README_ESPAÑOL.md) | [Português](./README_PORTUGUÊS.md)

## Índice

- [Por que o Blacksmith existe](#por-que-o-blacksmith-existe)
- [O que ele pode administrar](#o-que-ele-pode-administrar)
- [O que ele deliberadamente não faz](#o-que-ele-deliberadamente-não-faz)
- [Arquitetura](#arquitetura)
- [Capturas de tela](#capturas-de-tela)
- [Início rápido para usuários experientes](#início-rápido-para-usuários-experientes)
- [Guia completo de configuração](#guia-completo-de-configuração)
- [Desenvolvimento](#desenvolvimento)
- [Mapa do repositório](#mapa-do-repositório)
- [Segurança](#segurança)
- [Licença](#licença)
- [Apoie o projeto](#apoie-o-projeto)

Blacksmith é um pequeno painel de controle para desktop e um agente de servidor para implantar
aplicações simples em servidores VPS bare-metal.

Ele foi criado para quem quer a praticidade de um painel de deploy sem transformar um VPS de
4 GB em outro projeto de infraestrutura. O agente é escrito em Rust, roda como um serviço do
systemd e permanece privado no servidor. O cliente desktop chega até ele por um túnel SSH, então
a API de administração não precisa ser exposta à Internet.

O Blacksmith tem um escopo intencionalmente limitado. Ele não é uma PaaS genérica, uma
plataforma de contêineres ou um orquestrador multi-host. Ele administra algumas aplicações em
um servidor sob seu controle usando as ferramentas do sistema que já estão disponíveis.

## Por que o Blacksmith existe

Alternativas self-hosted inspiradas em Heroku ou Vercel podem ser úteis, mas frequentemente
trazem seus próprios bancos de dados, filas, proxies, runtimes de contêineres ou camadas de
orquestração. Em um VPS com 4 GB de RAM e uma vCPU, essa sobrecarga é o orçamento inteiro.

O Blacksmith foi criado para resolver um problema mais simples:

> Implantar e operar alguns projetos de forma confiável sem repetir manualmente os mesmos
> comandos SSH em cada release e sem instalar uma plataforma pesada para fazer isso.

Rust foi escolhido para o agente porque a parte que roda no servidor precisa ser pequena,
previsível e cuidadosa com execução de processos, credenciais, systemd e limites do sistema de
arquivos. O resultado mantém o deploy próximo do host e oferece um fluxo de trabalho desktop
prático.

## O que ele pode administrar

O Blacksmith é adequado para aplicações como:

- Serviços Python gerenciados pelo systemd
- Aplicações e APIs Node.js
- Binários e serviços Rust
- Sites estáticos e frontends compilados
- APIs, serviços WebSocket ou uma aplicação com frontend, API e endpoint WebSocket

Um deploy pode clonar uma branch, executar comandos de preparação, ativar uma nova release,
iniciar o serviço e manter a release anterior para rollback. A saída estática pode ser
publicada pela integração incluída com nginx. Arquivos de ambiente, logs, limites de recursos
e informações básicas do host são administrados pelo mesmo cliente.

## O que ele deliberadamente não faz

O Blacksmith não usa nem administra:

- Docker, Podman, Kubernetes ou outros runtimes de contêineres
- Funções serverless
- Orquestração multi-host
- Aplicações distribuídas complexas
- Um endpoint público de administração
- Um substituto para systemd, nginx, Git ou OpenSSH

A restrição bare-metal faz parte do design, não é uma funcionalidade faltante. Se a aplicação
precisa de service mesh, scheduler distribuído, escalabilidade horizontal automática ou imagens
de contêiner isoladas, o Blacksmith não é a ferramenta certa.

## Arquitetura

O repositório é um workspace Cargo com quatro partes principais:

| Componente          | Função                                                          |
| ------------------- | --------------------------------------------------------------- |
| `agent/`            | Serviço Axum e Tokio instalado no VPS                           |
| `client/`           | Interface de usuário SvelteKit                                  |
| `client/src-tauri/` | Runtime desktop Tauri 2 e transporte SSH                        |
| `shared/`           | Tipos de API serializáveis compartilhados pelo agente e cliente |

O agente escuta por padrão em `127.0.0.1:4040`. O aplicativo desktop abre um encaminhamento
local SSH para esse endereço e envia as requisições pelo túnel. O OpenSSH autentica a conexão;
o Blacksmith não adiciona um segundo sistema de login com JWT ou sessões de navegador.

A superfície HTTP pública fica limitada ao endpoint de webhook do GitHub quando os webhooks
estão habilitados. A administração permanece atrás do SSH.

### Modelo de segurança

- O agente não executa comandos da aplicação diretamente; as templates de serviço do systemd
  fazem isso.
- Os serviços das aplicações usam uma conta separada sem privilégios nas instalações de
  servidor.
- systemd e polkit restringem quais unidades o agente pode controlar.
- As credenciais de deploy passam por credenciais do systemd, não por argumentos de linha de
  comando ou URLs de repositório.
- As chaves privadas permanecem no runtime desktop e são armazenadas no cofre de credenciais
  do sistema usando criptografia de envelope.
- Os processos dos serviços ficam confinados ao seu próprio diretório de estado pelo sandbox
  do systemd.
- Os limites de recursos são aplicados ao cgroup do serviço, não apenas ao processo principal.

Revise as templates de deploy e as regras do polkit antes de usar o Blacksmith em um host de
produção. Elas fazem parte da fronteira de segurança.

## Capturas de tela

O repositório inclui as capturas visuais geradas pela suíte de regressão do cliente. Estas são
visões representativas do fluxo de trabalho desktop:

![Lista de servidores](../client/e2e/__screenshots__/visual/server-list--lista-dark.png)

![Painel do serviço](../client/e2e/__screenshots__/visual/service-dashboard--en-marcha-dark.png)

![Formulário de deploy](../client/e2e/__screenshots__/visual/deployment-form--definido-dark.png)

![Logs ao vivo](../client/e2e/__screenshots__/visual/log-viewer--en-vivo-dark.png)

A suíte visual também cobre modo claro, falhas de conexão, confirmação da chave do host,
histórico de deploys, edição do ambiente, limites de recursos, sites nginx e serviços estáticos.

## Início rápido para usuários experientes

### Compilar o agente

O Blacksmith requer Rust 1.97 ou mais recente. O repositório fixa o toolchain em
[`rust-toolchain.toml`](../rust-toolchain.toml).

```sh
git clone https://github.com/HormigaDev/blacksmith.git
cd blacksmith
cargo build --release -p agent
```

O binário gerado é `target/release/blacksmith-agent`.

### Instalar no servidor

A versão curta, como root no VPS e a partir da raiz do repositório (cada passo é explicado no
[guia completo](#guia-completo-de-configuração)):

```sh
apt install git openssh-client polkitd
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$user"
done
install -D -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 deploy/agent.env /etc/blacksmith/agent.env
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
install -m 0644 deploy/systemd/*.service deploy/blacksmith.service /etc/systemd/system/
systemctl daemon-reload
sh deploy/polkit/verify.sh && sh deploy/systemd/verify-sandbox.sh
systemctl enable --now blacksmith.service
```

Opcional: `sh deploy/nginx/install.sh` para sites e
`sh deploy/nginx/install-webhooks.sh <domínio>` para fazer deploy a cada push. Nunca substitua a
regra do polkit por uma entrada ampla no `sudoers`.

### Executar o cliente desktop

```sh
cd client
pnpm install
pnpm tauri dev
```

Adicione o servidor no cliente, aceite sua chave de host na primeira conexão e configure o
repositório e a branch do serviço. Para desenvolvimento local sem VPS, o agente pode usar a
instância systemd do usuário atual:

```sh
BLACKSMITH_SYSTEMD_SCOPE=user cargo run -p agent
```

O modo local não oferece o sandbox do servidor. Use-o somente com repositórios e comandos em
que você confia.

## Guia completo de configuração

Todos os comandos dos passos 1 a 8 rodam **como root no VPS, a partir da raiz de uma cópia deste
repositório** (os scripts e as unidades são lidos de `deploy/`).

### Requisitos

No VPS (Debian 12+ ou Ubuntu 22.04+; em outras distribuições os pacotes podem ter outro nome):

- systemd, polkit (`polkitd`), Git e o cliente OpenSSH (`git`, `openssh-client`): o agente
  gerencia as unidades pelo polkit e os deploys clonam com `git` (e `ssh` para as chaves de
  deploy).
- Um servidor SSH que permita o encaminhamento TCP (veja o passo 6).
- Para sites: `nginx` e, para HTTPS, `certbot`. Um domínio cujo DNS aponte para o VPS e as portas
  80 e 443 abertas.
- Para compilar no próprio VPS: Rust 1.97+ e um compilador C (`build-essential`).

No desktop:

- Rust 1.97 ou mais recente
- Node.js 24.14.0
- pnpm 11
- Os requisitos de sistema do Tauri 2 para sua plataforma (no Linux, os pacotes de
  desenvolvimento do GTK 3 e do WebKitGTK 4.1 do [guia de requisitos do Tauri](https://v2.tauri.app/start/prerequisites/))

```sh
apt install git openssh-client polkitd
```

### 1. Compilar o agente

```sh
cargo build --release -p agent
```

Compile no VPS, ou numa máquina com a mesma arquitetura e uma glibc igual ou mais antiga, e copie
`target/release/blacksmith-agent` junto com o diretório `deploy/`.

### 2. Criar as contas de serviço

Três contas, para que nenhum código implantado rode com as permissões do agente:

| Conta | Executa | Pode |
|---|---|---|
| `blacksmith` | o agente | gerenciar as próprias unidades pelo polkit |
| `blacksmith-apps` | os serviços e seus comandos de preparação | escrever só no diretório do seu serviço |
| `blacksmith-fetch` | o clone do repositório | ler as credenciais do deploy, nada mais |

```sh
for user in blacksmith blacksmith-apps blacksmith-fetch; do
    useradd --system --no-create-home --home-dir /nonexistent \
        --shell /usr/sbin/nologin "$user"
done
```

### 3. Instalar o agente e sua configuração

```sh
install -D -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -d -o root -g blacksmith -m 0750 /etc/blacksmith
install -o root -g blacksmith -m 0640 deploy/agent.env /etc/blacksmith/agent.env
```

O binário pertence ao root, assim o agente nunca pode substituir o próprio executável.
`/etc/blacksmith/agent.env` precisa existir mesmo com todas as linhas comentadas: os valores
padrão são os de produção. Ele aceita as variáveis `BLACKSMITH_*` documentadas em
[`.env.template`](../.env.template); uma desconhecida impede o agente de iniciar. Segredos nunca
vão neste arquivo.

### 4. Instalar os modelos de serviço e o polkit

```sh
install -m 0644 deploy/systemd/*.service /etc/systemd/system/
install -m 0644 deploy/blacksmith.service /etc/systemd/system/
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
systemctl daemon-reload
```

Verifique os dois antes de iniciar o agente:

```sh
sh deploy/polkit/verify.sh            # termina com «regla correcta»
sh deploy/systemd/verify-sandbox.sh   # termina com «Sandbox correcto.»
```

`verify.sh` cria unidades temporárias e confere o que o agente pode e não pode fazer.
`verify-sandbox.sh` inicia um serviço descartável com o mesmo sandbox de um real e confere que ele
não vê outros serviços, os dados do agente, o D-Bus nem processos de outros usuários. Os dois
limpam o que criam.

### 5. Iniciar o agente

```sh
systemctl enable --now blacksmith.service
systemctl status blacksmith.service
journalctl -u blacksmith.service -n 50
```

O agente escuta só em `127.0.0.1:4040` e, ao iniciar, registra se o polkit o autoriza. O banco de
dados é criado em `/var/lib/blacksmith/agent.db`. Não abra a porta 4040 no firewall: nada fora do
servidor precisa dela.

### 6. Permitir o túnel SSH

O cliente desktop chega ao agente por um túnel SSH até `127.0.0.1:4040`, usando a chave SSH de
qualquer conta do VPS (não precisa ser root). O servidor SSH deve permitir o encaminhamento TCP
para essa conta. Ele permite por padrão; se `/etc/ssh/sshd_config` (ou um arquivo em
`sshd_config.d/`) tiver `AllowTcpForwarding no`, mude para `local` para essa conta, por exemplo:

```text
Match User deploy
    AllowTcpForwarding local
```

e `systemctl reload ssh`. Na primeira conexão o cliente mostra a impressão digital da chave do
servidor; compare com `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` antes de aceitar.

### 7. Sites com nginx (opcional)

Só é necessário para serviços que servem um site (arquivos estáticos ou um proxy reverso):

```sh
apt install nginx certbot
sh deploy/nginx/install.sh
```

O script instala a configuração comum, os snippets de segurança, os modelos editáveis em
`/etc/blacksmith/nginx/templates/` e as duas unidades que rodam como root em nome do agente
(validar e recarregar o nginx, pedir certificados), cada uma protegida por um script que valida
de novo o que o agente escreveu. Não precisa de nenhum domínio e pode ser repetido a cada
atualização.

Se ele avisar que já existe outro `default_server` (normalmente o site `default` do Debian),
remova-o para que hosts desconhecidos não recebam esse site e rode o script de novo:

```sh
rm /etc/nginx/sites-enabled/default
sh deploy/nginx/install.sh
```

Os domínios são configurados por serviço, na aba **Web** do cliente. Antes de salvar um site,
aponte o DNS do domínio (registros A/AAAA) para o VPS e abra as portas 80 e 443. Com HTTPS
ativado, o certificado é pedido em segundo plano e renovado pelo próprio timer do certbot.

### 8. Deploy a cada push com webhooks do GitHub (opcional)

Requer o passo 7 e um domínio próprio para os webhooks (por exemplo `hooks.exemplo.com`) cujo DNS
já aponte para o VPS:

```sh
sh deploy/nginx/install-webhooks.sh hooks.exemplo.com voce@exemplo.com
```

O script gera o segredo do webhook (legível só pelo root, entregue ao agente como credencial do
systemd por um drop-in, então você nunca edita a unidade), obtém o certificado do domínio,
publica só `POST /api/webhooks/github` e mostra os valores para colocar no GitHub. Depois, em
cada repositório: **Settings → Webhooks → Add webhook**:

- **Payload URL**: `https://hooks.exemplo.com/api/webhooks/github`
- **Content type**: `application/json`
- **Secret**: o valor mostrado pelo script (também em `/etc/blacksmith/credentials/github-webhook-secret`)
- **Events**: *Just the push event*

Cada push numa branch faz o deploy de todos os serviços configurados com esse repositório e essa
branch que tenham **Deploy a cada push** ativado (na aba **Deploy**); vem ativado por padrão.
Requisições com assinatura inválida são rejeitadas; a API de administração nunca é publicada.

Se você usa uma GitHub App própria em vez de webhooks de repositório, coloque a mesma URL e o
mesmo segredo na configuração de webhook dela. A App padrão BBEL Blacksmith não serve para isso,
porque você não controla o webhook dela. O Blacksmith nunca precisa da chave privada de nenhuma
GitHub App.

Quando um serviço usa uma chave de deploy, uma GitHub App própria com **Administration: Read and
write** registra automaticamente a chave somente leitura gerada. Com a App padrão BBEL Blacksmith
o cliente usa o fluxo manual de copiar e colar, porque essa permissão não é concedida a ela de
propósito.

### 9. Registrar e fazer deploy de um serviço

No cliente desktop:

1. Adicione o VPS e conecte por SSH.
2. Registre um serviço com um identificador em minúsculas.
3. Escolha o repositório do GitHub e a branch.
4. Adicione comandos de preparação como `python -m venv .venv`, `npm ci` ou `cargo build --release`.
5. Defina o comando de início do serviço, se ele tiver um processo.
6. Opcionalmente, escolha uma pasta pública para um site estático e configure o domínio na aba
   **Web**.
7. Faça o deploy e confira o status e os logs do serviço.

Cada deploy tem seu próprio diretório de versão. A versão anterior continua disponível para
voltar a ela se uma compilação ou uma inicialização nova falhar.

### Atualizar

Compile o agente de novo e repita as instalações, que são idempotentes:

```sh
install -o root -g root -m 0755 target/release/blacksmith-agent /opt/blacksmith/bin/blacksmith-agent
install -m 0644 deploy/systemd/*.service deploy/blacksmith.service /etc/systemd/system/
install -o root -g root -m 0644 deploy/polkit/10-blacksmith.rules /etc/polkit-1/rules.d/
systemctl daemon-reload && systemctl restart blacksmith.service
sh deploy/nginx/install.sh                                  # se você usa sites
sh deploy/nginx/install-webhooks.sh hooks.exemplo.com       # se você usa webhooks
```

O banco de dados é migrado sozinho. Seus modelos do nginx editados e o segredo do webhook são
mantidos.

## Desenvolvimento

Na raiz do repositório:

```sh
cargo check --workspace
cargo test -p agent -p shared
cargo clippy -p agent -p shared --all-targets -- -D warnings
```

Para o cliente:

```sh
cd client
pnpm install --frozen-lockfile
pnpm check
pnpm test
pnpm test:e2e:app
```

O fluxo do cliente é executado manualmente pelo GitHub Actions porque suas suítes de navegador
e regressão visual são intencionalmente mais pesadas que uma verificação por commit.

## Mapa do repositório

```text
agent/                 Agente VPS e integração com systemd
client/                Interface SvelteKit, testes e capturas visuais
client/src-tauri/      Runtime desktop Tauri e implementação SSH
shared/                Modelos de API compartilhados em Rust
deploy/                Arquivos de systemd, nginx, polkit e instalação
```

## Segurança

O Blacksmith exige conhecimento cuidadoso de administração de sistemas. Leia a
[política de segurança](../SECURITY.md) antes de implantá-lo em um host de produção. Ela
explica as versões `v1.0.x` com suporte, o processo privado para reportar vulnerabilidades e a
responsabilidade do operador de configurar corretamente SSH, systemd, polkit, credenciais,
firewall e comandos de deploy.

## Licença

O Blacksmith é distribuído sob a [Licença MIT](../LICENSE).

## Apoie o projeto

Se o Blacksmith economiza seu tempo ou torna um VPS pequeno mais prático, você pode apoiar sua
manutenção contínua:

[![Doar com PayPal](https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif)](https://www.paypal.com/donate/?hosted_button_id=UCL7EE2G44KPQ)
