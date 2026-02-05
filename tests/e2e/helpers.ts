import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

export function generateToken(sub: string, expiresInSeconds: number = 60): string {
  const privateKeyPath = path.resolve(__dirname, '../../certs/private.pem');
  const privateKey = fs.readFileSync(privateKeyPath, 'utf-8');

  const header = Buffer.from(JSON.stringify({ alg: 'RS256', typ: 'JWT' })).toString('base64url');
  const now = Math.floor(Date.now() / 1000);
  const payload = Buffer.from(JSON.stringify({
    sub,
    exp: now + expiresInSeconds,
    iat: now,
  })).toString('base64url');

  const signature = crypto.sign('RSA-SHA256', Buffer.from(`${header}.${payload}`), privateKey);
  return `${header}.${payload}.${signature.toString('base64url')}`;
}

export function generateExpiredToken(sub: string): string {
  // Use -300 (5 minutes ago) to exceed jsonwebtoken's default 60s leeway
  return generateToken(sub, -300);
}
