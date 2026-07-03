# Moveme Storage

Serviço de object storage em Rust, inspirado no **Firebase Storage** e compatível com a API REST do **Google Cloud Storage**.

## Funcionalidades

- **Buckets** — criar, listar, obter e deletar buckets
- **Objetos** — upload, download, delete, listagem com prefixo e paginação
- **Metadados** — content-type, MD5, metadados customizados (`x-goog-meta-*`)
- **Upload resumível** — sessões com chunks para arquivos grandes
- **URLs assinadas** — acesso temporário sem token
- **Autenticação Firebase** — valida o ID Token do usuário logado no Firebase Auth
- **Regras de segurança** — DSL estilo Firebase Storage Rules (JSON)
- **Backend extensível** — filesystem local (pronto para S3/GCS)

## Arquitetura

```
storage/
├── crates/
│   ├── storage-core/    # Engine de storage, SQLite, backend filesystem
│   ├── storage-auth/    # Firebase Auth, regras de segurança, URLs assinadas
│   ├── storage-api/     # API REST (Axum)
│   └── storage-server/  # Binário principal
├── config/
│   ├── default.toml
│   └── security_rules.json
└── data/                # Dados persistidos (gitignored)
```

## Início rápido

### Pré-requisitos

- Rust 1.75+

### Executar

```bash
cargo run -p storage-server
```

O servidor sobe em `http://localhost:8091`.

### Swagger / OpenAPI

Documentação interativa disponível em:

- **UI:** http://localhost:8091/swagger-ui
- **JSON:** http://localhost:8091/api-docs/openapi.json

Na UI, clique em **Authorize** e cole o Firebase ID Token (`Bearer` é adicionado automaticamente).

Guia de integração para clientes: [docs/INTEGRATION.md](docs/INTEGRATION.md) (Flutter e Node.js).

### Variáveis de ambiente

Copie `.env.example` para `.env` ou use `config/default.toml`:

| Variável | Descrição | Padrão |
|----------|-----------|--------|
| `STORAGE_HOST` | Host de bind | `0.0.0.0` |
| `STORAGE_PORT` | Porta | `8091` |
| `STORAGE_DATA_DIR` | Diretório de dados | `./data` |
| `STORAGE_FIREBASE_PROJECT_ID` | ID do projeto Firebase | *(obrigatório)* |
| `STORAGE_MAX_UPLOAD_SIZE` | Tamanho máximo (bytes); omitir ou `0` = sem limite | sem limite |
| `STORAGE_BACKUP_DIR` | Pasta dos backups | `./backups` |
| `STORAGE_BACKUP_RETENTION_COUNT` | Backups mantidos | `10` |
| `STORAGE_BACKUP_AUTO_INTERVAL_HOURS` | Backup automático (0 = off) | `0` |
| `STORAGE_AUTO_CREATE_BUCKETS` | Criar buckets automaticamente no startup e no primeiro upload | `true` |
| `STORAGE_DEFAULT_BUCKET_LOCATION` | Região dos buckets auto-criados | `us-central1` |
| `FIREBASE_STORAGE_BUCKET` | Bucket padrão Firebase (criado no startup se auto_create ativo) | — |

## Backup

Backup completo de `storage.db` + pasta `objects/` em arquivos `.tar.gz` em `./backups/`.

```bash
# Criar (admin)
curl -X POST http://localhost:8091/v1/backups \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"label": "manual"}'

# Listar
curl http://localhost:8091/v1/backups -H "Authorization: Bearer $TOKEN"

# Restaurar (cria backup de segurança antes — reinicie o servidor depois)
curl -X POST http://localhost:8091/v1/backups/{id}/restore \
  -H "Authorization: Bearer $TOKEN"
```

Backup automático: `backup_auto_interval_hours = 24` em `config/default.toml`.

## API

### Autenticação (Firebase)

O cliente envia o **ID Token** obtido após login no Firebase Auth:

```javascript
// JavaScript (Firebase SDK)
const token = await firebase.auth().currentUser.getIdToken();
```

```bash
# Todas as requisições autenticadas usam:
curl -H "Authorization: Bearer <firebase-id-token>" ...
```

Configure `STORAGE_FIREBASE_PROJECT_ID` com o ID do seu projeto Firebase (ex: `moveme-app`).

Para usuários admin, defina a custom claim via Firebase Admin SDK:

```javascript
admin.auth().setCustomUserClaims(uid, { admin: true });
```

### Buckets

```bash
TOKEN="<firebase-id-token>"

# Criar bucket
curl -X POST http://localhost:8091/v0/b \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "meu-bucket", "location": "us-central1"}'

# Listar buckets
curl http://localhost:8091/v0/b -H "Authorization: Bearer $TOKEN"
```

### Upload de objeto

```bash
TOKEN="<firebase-id-token>"

# Upload simples (POST)
curl -X POST "http://localhost:8091/v0/b/meu-bucket/o?name=public/foto.jpg" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: image/jpeg" \
  -H "x-goog-meta-author: joao" \
  --data-binary @foto.jpg
```

### Download

```bash
# Metadados
curl "http://localhost:8091/v0/b/meu-bucket/o/public%2Ffoto.jpg"

# Conteúdo (alt=media)
curl "http://localhost:8091/v0/b/meu-bucket/o/public%2Ffoto.jpg?alt=media" -o foto.jpg
```

### Listar objetos

```bash
curl "http://localhost:8091/v0/b/meu-bucket/o?prefix=public/" \
  -H "Authorization: Bearer $TOKEN"
```

### Upload resumível

```bash
# 1. Iniciar sessão
curl -X PUT "http://localhost:8091/v0/b/meu-bucket/o?name=private/grande.zip" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/zip" \
  -H "x-upload-content-length: 50000000"

# 2. Enviar chunks
curl -X PUT "http://localhost:8091/v0/b/meu-bucket/o/upload?upload_id=SESSION_ID" \
  -H "Content-Type: application/octet-stream" \
  -H "Content-Range: bytes 0-1048575" \
  --data-binary @chunk1.bin
```

### URL assinada

```bash
curl -X POST http://localhost:8091/v1/signed-url \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "bucket": "meu-bucket",
    "object_path": "public/foto.jpg",
    "method": "GET",
    "expires_in_secs": 3600
  }'
```

### Deletar objeto

```bash
curl -X DELETE "http://localhost:8091/v0/b/meu-bucket/o/public%2Ffoto.jpg" \
  -H "Authorization: Bearer $TOKEN"
```

## Regras de segurança

Arquivo `config/security_rules.json`:

```json
{
  "rules": [
    {
      "path": "public/**",
      "read": true,
      "write": "request.auth != null",
      "delete": "request.auth.role == 'admin'"
    },
    {
      "path": "private/**",
      "read": "request.auth != null",
      "write": "request.auth != null",
      "delete": "request.auth.role == 'admin'"
    }
  ]
}
```

Expressões suportadas:

- `request.auth != null` — requer autenticação
- `request.auth.admin == true` — requer custom claim `admin` no Firebase
- `request.auth.uid == 'userId'` — requer usuário específico
- `true` / `false` — acesso público ou negado

## Endpoints

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/health` | Health check |
| GET | `/swagger-ui` | Documentação Swagger (UI) |
| GET | `/api-docs/openapi.json` | Spec OpenAPI em JSON |
| POST | `/v0/b` | Criar bucket |
| GET | `/v0/b` | Listar buckets |
| GET | `/v0/b/{bucket}` | Obter bucket |
| DELETE | `/v0/b/{bucket}` | Deletar bucket |
| POST | `/v0/b/{bucket}/o?name=...` | Upload simples |
| PUT | `/v0/b/{bucket}/o?name=...` | Iniciar upload resumível |
| GET | `/v0/b/{bucket}/o` | Listar objetos |
| GET | `/v0/b/{bucket}/o/{path}` | Metadados do objeto |
| GET | `/v0/b/{bucket}/o/{path}?alt=media` | Download |
| PATCH | `/v0/b/{bucket}/o/{path}` | Atualizar metadados |
| DELETE | `/v0/b/{bucket}/o/{path}` | Deletar objeto |
| POST | `/v1/signed-url` | Gerar URL assinada |
| POST | `/v1/backups` | Criar backup (admin) |
| GET | `/v1/backups` | Listar backups (admin) |
| GET | `/v1/backups/{id}` | Detalhes do backup (admin) |
| DELETE | `/v1/backups/{id}` | Deletar backup (admin) |
| POST | `/v1/backups/{id}/restore` | Restaurar backup (admin) |

## Desenvolvimento

```bash
# Compilar
cargo build

# Testes
cargo test

# Release
cargo build --release
```

## Licença

MIT
