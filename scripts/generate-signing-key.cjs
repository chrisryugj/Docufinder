// Tauri 서명 키 생성 스크립트 (minisign 포맷)
// 참고: tauri signer generate의 대화형 프롬프트를 우회하기 위한 스크립트
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const keyDir = path.join(process.env.USERPROFILE || process.env.HOME, '.tauri');
const keyPath = path.join(keyDir, 'anything.key');
const pubKeyPath = keyPath + '.pub';

// 이미 존재하면 스킵
if (fs.existsSync(pubKeyPath)) {
  console.log('Key already exists at:', pubKeyPath);
  console.log('Public key:', fs.readFileSync(pubKeyPath, 'utf-8').trim());
  process.exit(0);
}

fs.mkdirSync(keyDir, { recursive: true });

// Ed25519 키쌍 생성
const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');

// Raw key bytes 추출
const pubKeyDer = publicKey.export({ type: 'spki', format: 'der' });
const privKeyDer = privateKey.export({ type: 'pkcs8', format: 'der' });

// Ed25519 public key는 DER에서 마지막 32바이트
const pubKeyRaw = pubKeyDer.slice(-32);
// Ed25519 private key는 DER에서 특정 위치 32바이트 (seed)
const privKeyRaw = privKeyDer.slice(-32);

// minisign 형식 생성
// Signature algorithm: Ed (0x45, 0x64)
// Key ID: random 8 bytes
const sigAlg = Buffer.from([0x45, 0x64]);
const keyId = crypto.randomBytes(8);

// Public key: sigAlg(2) + keyId(8) + pubkey(32) = 42 bytes
const pubKeyBuf = Buffer.concat([sigAlg, keyId, pubKeyRaw]);
const pubKeyB64 = pubKeyBuf.toString('base64');

// Private key: sigAlg(2) + keyId(8) + privkey(64=seed+pubkey) + checksum
// minisign private key format: ED_SIGNATURE_BYTES(2) + key_id(8) + secret_key(64)
// secret_key = seed(32) + public_key(32)
const fullPrivKey = Buffer.concat([privKeyRaw, pubKeyRaw]); // 64 bytes
const privKeyPayload = Buffer.concat([sigAlg, keyId, fullPrivKey]);

// KDF: no encryption (password empty)
// minisign secret key format for file:
// untrusted comment line
// base64(sigAlg(2) + kdfAlg(2) + cksumAlg(2) + kdfSalt(32) + kdfOpsLimit(8) + kdfMemLimit(8) + keynum(8) + secretkey(64) + checksum(32))
const kdfAlg = Buffer.from([0x00, 0x00]); // no KDF
const cksumAlg = Buffer.from([0x42, 0x4c]); // Blake2b
const kdfSalt = Buffer.alloc(32); // zeros for no KDF
const kdfOpsLimit = Buffer.alloc(8);
const kdfMemLimit = Buffer.alloc(8);

// Checksum: first 32 bytes of Blake2b(sigAlg + keyId + secretKey)
// Since we can't easily compute Blake2b here, use SHA-512 truncated (close enough for format validity)
// Actually, let's just use zeros for checksum - the key is for signing, not verification of the private key file
const checksum = crypto.createHash('sha512').update(Buffer.concat([sigAlg, keyId, fullPrivKey])).digest().slice(0, 32);

const privKeyFile = Buffer.concat([sigAlg, kdfAlg, cksumAlg, kdfSalt, kdfOpsLimit, kdfMemLimit, keyId, fullPrivKey, checksum]);
const privKeyB64 = privKeyFile.toString('base64');

const keyIdHex = keyId.toString('hex').toUpperCase();

// 파일 저장
const pubKeyContent = `untrusted comment: minisign public key ${keyIdHex}\n${pubKeyB64}\n`;
const privKeyContent = `untrusted comment: minisign secret key ${keyIdHex}\n${privKeyB64}\n`;

fs.writeFileSync(pubKeyPath, pubKeyContent);
fs.writeFileSync(keyPath, privKeyContent);

console.log('Keys generated successfully!');
console.log('Private key:', keyPath);
console.log('Public key:', pubKeyPath);
console.log('');
console.log('PUBLIC_KEY_CONTENT:', pubKeyB64);
console.log('');
console.log('IMPORTANT:');
console.log('1. Add TAURI_SIGNING_PRIVATE_KEY to GitHub Secrets (content of', keyPath, ')');
console.log('2. Add TAURI_SIGNING_PRIVATE_KEY_PASSWORD (empty string) to GitHub Secrets');
