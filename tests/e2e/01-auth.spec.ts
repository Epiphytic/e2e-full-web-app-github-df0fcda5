import { test, expect } from '@playwright/test';
import { generateToken, generateExpiredToken } from './helpers';

test.describe('Authentication', () => {
  test('shows login page when not authenticated', async ({ page }) => {
    await page.goto('/login');
    await expect(page.locator('h1')).toContainText('SQLite Editor');
    await expect(page.locator('textarea[name="token"]')).toBeVisible();
  });

  test('login with valid short-lived JWT token', async ({ page }) => {
    const token = generateToken('testuser', 120);
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');
    await expect(page.locator('text=Logged in as testuser')).toBeVisible();
  });

  test('login with expired token shows error', async ({ page }) => {
    const token = generateExpiredToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    // The form POST returns HTML directly (not a redirect) on error,
    // so we need to wait for the response to load
    await Promise.all([
      page.waitForNavigation({ waitUntil: 'domcontentloaded' }),
      page.locator('button[type="submit"]').click(),
    ]);
    await expect(page.locator('.error')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('.error')).toContainText('Invalid token');
  });

  test('logout clears session', async ({ page }) => {
    const token = generateToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');

    await page.locator('a:text("Logout")').click();
    await expect(page).toHaveURL(/\/login/);
  });

  test('.well-known/jwks.json endpoint returns public key', async ({ request }) => {
    const response = await request.get('/.well-known/jwks.json');
    expect(response.ok()).toBeTruthy();
    const jwks = await response.json();
    expect(jwks.keys).toHaveLength(1);
    expect(jwks.keys[0].kty).toBe('RSA');
    expect(jwks.keys[0].alg).toBe('RS256');
  });
});
