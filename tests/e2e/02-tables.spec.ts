import { test, expect } from '@playwright/test';
import { generateToken } from './helpers';

async function login(page: any) {
  const token = generateToken('testuser');
  await page.goto('/login');
  await page.locator('textarea[name="token"]').fill(token);
  await page.locator('button[type="submit"]').click();
  await expect(page).toHaveURL('/');
  // Wait for htmx to load the table list (replaces "Loading tables...")
  await page.waitForFunction(() => {
    const el = document.getElementById('table-list');
    return el && !el.textContent?.includes('Loading');
  }, { timeout: 10000 });
}

test.describe('Table Operations', () => {
  test('shows table list on dashboard', async ({ page }) => {
    await login(page);
    // The table list div should exist and be loaded (not "Loading...")
    await expect(page.locator('#table-list')).toBeVisible();
  });

  test('create a new table', async ({ page }) => {
    await login(page);
    await page.locator('input[name="table_name"]').fill('test_create');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();

    await expect(page.locator('#table-list')).toContainText('test_create', { timeout: 5000 });
  });

  test('drop a table', async ({ page }) => {
    await login(page);

    // First create a table to drop
    await page.locator('input[name="table_name"]').fill('to_drop');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('to_drop', { timeout: 5000 });

    // Set up dialog handler BEFORE clicking
    page.once('dialog', (dialog: any) => dialog.accept());

    // Drop the specific table and wait for htmx response
    const tableItem = page.locator('li', { has: page.locator('a[href="/tables/to_drop"]') });
    await Promise.all([
      page.waitForResponse((resp: any) => resp.url().includes('/api/tables/to_drop') && resp.status() === 200),
      tableItem.locator('button.danger').click(),
    ]);

    // Wait for htmx to swap the response
    await expect(page.locator('#table-list')).not.toContainText('to_drop', { timeout: 5000 });
  });

  test('navigate to table detail', async ({ page }) => {
    await login(page);

    // Create a table first
    await page.locator('input[name="table_name"]').fill('nav_test');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('nav_test', { timeout: 5000 });

    // Navigate directly to the table detail page
    await page.goto('/tables/nav_test');
    await expect(page.locator('h1')).toContainText('nav_test');
  });
});
