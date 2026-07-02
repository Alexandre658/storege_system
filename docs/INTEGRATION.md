# Integração Moveme Storage — Flutter e Node.js

Guia para conectar apps **Flutter** e backends **Node.js** ao Moveme Storage.

## Visão geral

| Item | Valor |
|------|-------|
| Base URL (dev) | `http://localhost:8091` |
| Base URL (servidor) | `http://SEU_HOST:8091` |
| Swagger | `{BASE_URL}/swagger-ui` |
| Autenticação | Firebase ID Token no header `Authorization: Bearer <token>` |
| Projeto Firebase | `moveme-1554316037072` |

Todas as rotas autenticadas exigem o **ID Token** do utilizador logado no Firebase Auth (não use API key nem custom JWT do servidor).

### Regras de caminhos (storage)

Organize os ficheiros por prefixo:

| Caminho | Leitura | Escrita | Exemplo |
|---------|---------|---------|---------|
| `public/**` | Pública | Utilizador autenticado | `public/banner.jpg` |
| `private/**` | Autenticado | Autenticado | `private/doc.pdf` |
| `users/{uid}/**` | Autenticado | Só o dono (`uid`) | `users/abc123/avatar.png` |

O motor de regras interpreta `{uid}` (ou `{userId}` no JSON de config) como o segmento do path e compara com `request.auth.uid` do token Firebase.

#### Normalização automática no upload

No **upload** (`POST /v0/b/{bucket}/o?name=...`) e **upload resumível** (`PUT` com `name=...`), se o parâmetro `name` **não contiver `/`**, o servidor prefixa automaticamente:

```
foto.jpg  →  users/{uid-do-token}/foto.jpg
```

Isto só aplica quando o pedido traz `Authorization: Bearer <token>` válido. Sem token, `foto.jpg` sozinho não casa com nenhuma regra → **403**.

| `name` no upload | Onde fica guardado | Escrita permitida? |
|------------------|--------------------|--------------------|
| `users/abc123/foto.jpg` | `users/abc123/foto.jpg` | Sim, se `token.uid == abc123` |
| `foto.jpg` (com token) | `users/{token.uid}/foto.jpg` | Sim (dono do token) |
| `public/foto.jpg` | `public/foto.jpg` | Sim (utilizador autenticado) |
| `foto.jpg` (sem token) | — | Não → 403 |

> **Download / delete / metadados** usam o path completo na URL (`/v0/b/{bucket}/o/{path}`). Use sempre o caminho final (ex.: `users/$uid/foto.jpg`), não apenas `foto.jpg`.

---

## Node.js

### Dependências

```bash
npm install firebase firebase-admin axios
# ou apenas no cliente:
npm install firebase axios
```

### Variáveis de ambiente

```env
STORAGE_BASE_URL=http://localhost:8091
FIREBASE_PROJECT_ID=moveme-1554316037072
```

### Cliente reutilizável

```javascript
// storage-client.js
const axios = require('axios');

class MovemeStorageClient {
  constructor(baseUrl, getIdToken) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.getIdToken = getIdToken; // async () => string
  }

  async headers(extra = {}) {
    const token = await this.getIdToken();
    return {
      Authorization: `Bearer ${token}`,
      ...extra,
    };
  }

  async upload(bucket, objectPath, data, contentType, metadata = {}) {
    const metaHeaders = Object.fromEntries(
      Object.entries(metadata).map(([k, v]) => [`x-goog-meta-${k}`, v])
    );

    const { data: result } = await axios.post(
      `${this.baseUrl}/v0/b/${bucket}/o`,
      data,
      {
        params: { name: objectPath },
        headers: await this.headers({
          'Content-Type': contentType,
          ...metaHeaders,
        }),
        maxBodyLength: Infinity,
      }
    );
    return result;
  }

  async downloadUrl(bucket, objectPath) {
    const encoded = encodeURIComponent(objectPath);
    return `${this.baseUrl}/v0/b/${bucket}/o/${encoded}?alt=media`;
  }

  async download(bucket, objectPath, getToken = true) {
    const encoded = encodeURIComponent(objectPath);
    const headers = getToken ? await this.headers() : {};
    const { data } = await axios.get(
      `${this.baseUrl}/v0/b/${bucket}/o/${encoded}`,
      { params: { alt: 'media' }, headers, responseType: 'arraybuffer' }
    );
    return data;
  }

  async list(bucket, { prefix, maxResults = 100 } = {}) {
    const { data } = await axios.get(`${this.baseUrl}/v0/b/${bucket}/o`, {
      params: { prefix, maxResults },
      headers: await this.headers(),
    });
    return data.items ?? [];
  }

  async delete(bucket, objectPath) {
    const encoded = encodeURIComponent(objectPath);
    await axios.delete(`${this.baseUrl}/v0/b/${bucket}/o/${encoded}`, {
      headers: await this.headers(),
    });
  }

  async signedUrl(bucket, objectPath, expiresInSecs = 3600, method = 'GET') {
    const { data } = await axios.post(
      `${this.baseUrl}/v1/signed-url`,
      { bucket, object_path: objectPath, method, expires_in_secs: expiresInSecs },
      { headers: await this.headers({ 'Content-Type': 'application/json' }) }
    );
    return data.signed_url;
  }
}

module.exports = { MovemeStorageClient };
```

### Node.js — cliente web / Firebase Auth (browser ou Electron)

```javascript
const { initializeApp } = require('firebase/app');
const { getAuth, signInWithEmailAndPassword } = require('firebase/auth');
const { MovemeStorageClient } = require('./storage-client');
const fs = require('fs');

const firebaseConfig = {
  apiKey: process.env.FIREBASE_API_KEY,
  authDomain: 'moveme-1554316037072.firebaseapp.com',
  projectId: 'moveme-1554316037072',
  storageBucket: 'moveme-1554316037072.appspot.com',
};

const app = initializeApp(firebaseConfig);
const auth = getAuth(app);

async function main() {
  await signInWithEmailAndPassword(auth, 'user@example.com', 'password');

  const storage = new MovemeStorageClient(
    process.env.STORAGE_BASE_URL ?? 'http://localhost:8091',
    () => auth.currentUser.getIdToken()
  );

  const uid = auth.currentUser.uid;
  const bucket = 'meu-bucket';

  // Upload para pasta do utilizador (recomendado — path explícito)
  const file = fs.readFileSync('./foto.jpg');
  const meta = await storage.upload(
    bucket,
    `users/${uid}/foto.jpg`,
    file,
    'image/jpeg',
    { author: 'joao' }
  );
  console.log('Upload OK:', meta);

  // Alternativa: só o nome do ficheiro — servidor guarda em users/{uid}/foto.jpg
  await storage.upload(bucket, 'foto.jpg', file, 'image/jpeg');

  // Listar ficheiros do utilizador
  const items = await storage.list(bucket, { prefix: `users/${uid}/` });
  console.log('Ficheiros:', items.map((i) => i.name));

  // Download
  const bytes = await storage.download(bucket, `users/${uid}/foto.jpg`);
  fs.writeFileSync('./download.jpg', Buffer.from(bytes));
}

main().catch(console.error);
```

### Node.js — backend com token do cliente (Express)

O mobile/app envia o Firebase ID Token; o backend repassa ao storage:

```javascript
const express = require('express');
const multer = require('multer');
const { MovemeStorageClient } = require('./storage-client');

const app = express();
const upload = multer({ storage: multer.memoryStorage() });

app.post('/api/upload', upload.single('file'), async (req, res) => {
  const idToken = req.headers.authorization?.replace('Bearer ', '');
  if (!idToken) return res.status(401).json({ error: 'Token em falta' });

  const storage = new MovemeStorageClient(
    process.env.STORAGE_BASE_URL,
    async () => idToken
  );

  try {
    const uid = req.body.uid; // ou extrair do token decodificado
    const result = await storage.upload(
      'meu-bucket',
      `users/${uid}/${req.file.originalname}`,
      req.file.buffer,
      req.file.mimetype
    );
    res.json(result);
  } catch (err) {
    res.status(err.response?.status ?? 500).json(err.response?.data ?? { error: err.message });
  }
});

app.listen(3000);
```

### Node.js — Admin (criar bucket, backups)

Requer custom claim `admin: true` no Firebase:

```javascript
// Definir admin (executar uma vez com service account)
const admin = require('firebase-admin');
admin.initializeApp({ credential: admin.credential.cert(serviceAccount) });
await admin.auth().setCustomUserClaims('UID_DO_ADMIN', { admin: true });
```

```javascript
const axios = require('axios');

async function createBucket(idToken) {
  const { data } = await axios.post(
    `${process.env.STORAGE_BASE_URL}/v0/b`,
    { name: 'meu-bucket', location: 'us-central1' },
    { headers: { Authorization: `Bearer ${idToken}` } }
  );
  return data;
}
```

---

## Flutter

### Dependências (`pubspec.yaml`)

```yaml
dependencies:
  firebase_core: ^3.8.0
  firebase_auth: ^5.3.0
  http: ^1.2.0
  # opcional para ficheiros locais:
  path: ^1.9.0
```

### Inicialização Firebase

```dart
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_auth/firebase_auth.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await Firebase.initializeApp(
    options: const FirebaseOptions(
      apiKey: 'SUA_API_KEY',
      authDomain: 'moveme-1554316037072.firebaseapp.com',
      projectId: 'moveme-1554316037072',
      storageBucket: 'moveme-1554316037072.appspot.com',
      messagingSenderId: '123456789012',
      appId: '1:123456789012:web:abcdef1234567890',
    ),
  );
  runApp(const MyApp());
}
```

### Cliente Dart

```dart
// lib/moveme_storage_client.dart
import 'dart:convert';
import 'dart:typed_data';
import 'package:firebase_auth/firebase_auth.dart';
import 'package:http/http.dart' as http;

class MovemeStorageClient {
  MovemeStorageClient({
    required this.baseUrl,
    FirebaseAuth? auth,
  }) : _auth = auth ?? FirebaseAuth.instance;

  final String baseUrl;
  final FirebaseAuth _auth;

  Future<String> _token() async {
    final user = _auth.currentUser;
    if (user == null) throw Exception('Utilizador não autenticado');
    return user.getIdToken();
  }

  Future<Map<String, String>> _authHeaders([Map<String, String>? extra]) async {
    return {
      'Authorization': 'Bearer ${await _token()}',
      ...?extra,
    };
  }

  Uri _objectUri(String bucket, String objectPath, {Map<String, String>? query}) {
    final encoded = Uri.encodeComponent(objectPath);
    return Uri.parse('$baseUrl/v0/b/$bucket/o/$encoded')
        .replace(queryParameters: query);
  }

  /// Upload de bytes
  Future<Map<String, dynamic>> uploadBytes({
    required String bucket,
    required String objectPath,
    required Uint8List bytes,
    required String contentType,
    Map<String, String> metadata = const {},
  }) async {
    final uri = Uri.parse('$baseUrl/v0/b/$bucket/o').replace(
      queryParameters: {'name': objectPath},
    );

    final headers = await _authHeaders({
      'Content-Type': contentType,
      for (final e in metadata.entries) 'x-goog-meta-${e.key}': e.value,
    });

    final response = await http.post(uri, headers: headers, body: bytes);
    if (response.statusCode != 200) {
      throw StorageException(response.statusCode, response.body);
    }
    return jsonDecode(response.body) as Map<String, dynamic>;
  }

  /// URL pública de download (só funciona em paths `public/**` sem token)
  String publicDownloadUrl(String bucket, String objectPath) {
    return _objectUri(bucket, objectPath, query: {'alt': 'media'}).toString();
  }

  /// Download autenticado
  Future<Uint8List> download({
    required String bucket,
    required String objectPath,
  }) async {
    final uri = _objectUri(bucket, objectPath, query: {'alt': 'media'});
    final response = await http.get(uri, headers: await _authHeaders());
    if (response.statusCode != 200) {
      throw StorageException(response.statusCode, response.body);
    }
    return response.bodyBytes;
  }

  /// Listar objetos
  Future<List<Map<String, dynamic>>> list({
    required String bucket,
    String? prefix,
    int maxResults = 100,
  }) async {
    final uri = Uri.parse('$baseUrl/v0/b/$bucket/o').replace(
      queryParameters: {
        if (prefix != null) 'prefix': prefix,
        'maxResults': '$maxResults',
      },
    );
    final response = await http.get(uri, headers: await _authHeaders());
    if (response.statusCode != 200) {
      throw StorageException(response.statusCode, response.body);
    }
    final data = jsonDecode(response.body) as Map<String, dynamic>;
    return (data['items'] as List<dynamic>? ?? [])
        .cast<Map<String, dynamic>>();
  }

  /// Apagar objeto
  Future<void> delete({
    required String bucket,
    required String objectPath,
  }) async {
    final uri = _objectUri(bucket, objectPath);
    final response = await http.delete(uri, headers: await _authHeaders());
    if (response.statusCode != 204) {
      throw StorageException(response.statusCode, response.body);
    }
  }

  /// URL assinada temporária
  Future<String> signedUrl({
    required String bucket,
    required String objectPath,
    int expiresInSecs = 3600,
    String method = 'GET',
  }) async {
    final uri = Uri.parse('$baseUrl/v1/signed-url');
    final response = await http.post(
      uri,
      headers: await _authHeaders({'Content-Type': 'application/json'}),
      body: jsonEncode({
        'bucket': bucket,
        'object_path': objectPath,
        'method': method,
        'expires_in_secs': expiresInSecs,
      }),
    );
    if (response.statusCode != 200) {
      throw StorageException(response.statusCode, response.body);
    }
    return (jsonDecode(response.body) as Map<String, dynamic>)['signed_url'] as String;
  }
}

class StorageException implements Exception {
  StorageException(this.statusCode, this.body);
  final int statusCode;
  final String body;
  @override
  String toString() => 'StorageException($statusCode): $body';
}
```

### Flutter — exemplos de uso

```dart
import 'dart:io';
import 'dart:typed_data';
import 'package:firebase_auth/firebase_auth.dart';
import 'moveme_storage_client.dart';

class StorageService {
  final _client = MovemeStorageClient(
    baseUrl: 'http://SEU_SERVIDOR:8091', // ou http://10.0.2.2:8091 no emulador Android
  );

  static const bucket = 'meu-bucket';

  /// Caminho canónico na pasta do utilizador (use no download/delete)
  String userPath(String filename) {
    final uid = FirebaseAuth.instance.currentUser!.uid;
    return 'users/$uid/$filename';
  }

  Future<Map<String, dynamic>> uploadUserPhoto(File file) async {
    final bytes = await file.readAsBytes();

    // Opção A (recomendada): path explícito
    return _client.uploadBytes(
      bucket: bucket,
      objectPath: userPath('avatar.jpg'),
      bytes: bytes,
      contentType: 'image/jpeg',
      metadata: {'source': 'flutter'},
    );
  }

  /// Opção B: só o nome — upload vira users/{uid}/avatar.jpg no servidor
  Future<Map<String, dynamic>> uploadUserPhotoShort(File file) async {
    final bytes = await file.readAsBytes();
    return _client.uploadBytes(
      bucket: bucket,
      objectPath: 'avatar.jpg',
      bytes: bytes,
      contentType: 'image/jpeg',
    );
  }

  Future<Uint8List> downloadUserPhoto() async {
    return _client.download(
      bucket: bucket,
      objectPath: userPath('avatar.jpg'),
    );
  }

  /// Imagem pública — pode usar em Image.network sem token
  String publicImageUrl(String path) {
    return _client.publicDownloadUrl(bucket, 'public/$path');
  }
}
```

### Widget com imagem pública

```dart
Image.network(
  MovemeStorageClient(baseUrl: 'http://SEU_SERVIDOR:8091')
      .publicDownloadUrl('meu-bucket', 'public/logo.png'),
  errorBuilder: (_, __, ___) => const Icon(Icons.broken_image),
)
```

### Flutter — upload de ficheiro grande (resumível)

Para ficheiros > 10 MB, use upload resumível. O parâmetro `name` segue as mesmas regras de normalização (`foto.jpg` → `users/{uid}/foto.jpg`).

```dart
Future<void> resumableUpload({
  required String bucket,
  required String objectPath, // ex.: userPath('video.mp4') ou 'video.mp4'
  required Uint8List fileBytes,
  required String contentType,
}) async {
  final token = await FirebaseAuth.instance.currentUser!.getIdToken();
  final base = 'http://SEU_SERVIDOR:8091';

  // 1. Iniciar sessão
  final initUri = Uri.parse('$base/v0/b/$bucket/o').replace(
    queryParameters: {'name': objectPath},
  );
  final initRes = await http.put(
    initUri,
    headers: {
      'Authorization': 'Bearer $token',
      'Content-Type': contentType,
      'x-upload-content-length': '${fileBytes.length}',
    },
  );
  final session = jsonDecode(initRes.body);
  final uploadId = session['upload_id'] as String;

  // 2. Enviar em chunks de 1 MB
  const chunkSize = 1024 * 1024;
  for (var offset = 0; offset < fileBytes.length; offset += chunkSize) {
    final end = (offset + chunkSize < fileBytes.length)
        ? offset + chunkSize
        : fileBytes.length;
    final chunk = fileBytes.sublist(offset, end);

    await http.put(
      Uri.parse('$base/v0/b/$bucket/o/upload').replace(
        queryParameters: {'upload_id': uploadId},
      ),
      headers: {
        'Content-Type': 'application/octet-stream',
        'Content-Range': 'bytes $offset-${end - 1}',
      },
      body: chunk,
    );
  }
}
```

---

## Referência rápida de endpoints

| Operação | Método | URL |
|----------|--------|-----|
| Health | GET | `/health` |
| Upload | POST | `/v0/b/{bucket}/o?name={path}` |
| Download | GET | `/v0/b/{bucket}/o/{path}?alt=media` |
| Metadados | GET | `/v0/b/{bucket}/o/{path}` |
| Listar | GET | `/v0/b/{bucket}/o?prefix={prefix}` |
| Apagar | DELETE | `/v0/b/{bucket}/o/{path}` |
| URL assinada | POST | `/v1/signed-url` |
| Upload resumível | PUT | `/v0/b/{bucket}/o?name={path}` |
| Chunk upload | PUT | `/v0/b/{bucket}/o/upload?upload_id={id}` |

> `{path}` na URL deve estar **URL-encoded** (`public/foto.jpg` → `public%2Ffoto.jpg`).

---

## Erros comuns

| Código | Significado | Solução |
|--------|-------------|---------|
| 401 | Token em falta ou inválido | Chamar `getIdToken()` após login Firebase |
| 403 | Regra de segurança | Ver tabela abaixo |
| 404 | Objeto/bucket não existe | Confirmar bucket e caminho |
| 409 | Bucket já existe | Usar outro nome ou ignorar se já criado |

### 403 — regras de segurança

Mensagem típica: `{"message":"acesso negado pelas regras de segurança"}`.

| Causa | Exemplo | Correção |
|-------|---------|----------|
| Path sem prefixo válido | `name=doc.pdf` sem token | Enviar `Authorization: Bearer …` ou usar `public/doc.pdf` |
| Escrita na pasta de outro utilizador | Upload em `users/OUTRO_UID/foto.jpg` | Usar `users/${meuUid}/…` ou só `foto.jpg` com o token certo |
| Leitura em `private/**` sem auth | GET sem header | Incluir token Firebase |
| Path de download errado após upload curto | Upload `avatar.jpg`, download `avatar.jpg` | Download em `users/$uid/avatar.jpg` |

### Emulador Android

Use `http://10.0.2.2:8091` em vez de `localhost` para aceder ao host.

### iOS Simulator

`http://localhost:8091` funciona diretamente.

### Produção

Use HTTPS e configure `STORAGE_BASE_URL` com o domínio real do servidor.

---

## Checklist de implementação

- [ ] Firebase Auth configurado no app (mesmo projeto `moveme-1554316037072`)
- [ ] Bucket: use `Firebase.app().options.storageBucket` — com `auto_create_buckets=true` (padrão) o servidor cria automaticamente
- [ ] Uploads em `users/{uid}/…` (explícito) ou só o nome do ficheiro com token (normalização automática)
- [ ] Download/delete usam o path completo (`users/{uid}/ficheiro.ext`)
- [ ] Token renovado (`getIdToken(true)` se expirado)
- [ ] Imagens públicas em `public/` para `Image.network` sem auth
- [ ] Ficheiros grandes com upload resumível
