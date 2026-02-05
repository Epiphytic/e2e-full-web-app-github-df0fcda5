import { test, expect } from '@playwright/test';
import { generateToken } from './helpers';

let tableCounter = 0;

async function loginAndCreateTable(page: any): Promise<string> {
  tableCounter++;
  const tableName = `coltest_${tableCounter}`;

  const token = generateToken('testuser');
  await page.goto('/login');
  await page.locator('textarea[name="token"]').fill(token);
  await page.locator('button[type="submit"]').click();
  await expect(page).toHaveURL('/');

  // Wait for htmx to load the table list
  await page.waitForFunction(() => {
    const el = document.getElementById('table-list');
    return el && !el.textContent?.includes('Loading');
  }, { timeout: 10000 });

  // Create the test table
  await page.locator('input[name="table_name"]').fill(tableName);
  await page.locator('input[name="col_name_1"]').fill('id');
  await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
  await page.locator('button:text("Create Table")').click();
  await expect(page.locator('#table-list')).toContainText(tableName, { timeout: 5000 });

  // Navigate to table detail page
  await page.goto(`/tables/${tableName}`);

  // Wait for htmx to load the column list (replaces "Loading columns...")
  await page.waitForFunction(() => {
    const el = document.getElementById('column-list');
    return el && !el.textContent?.includes('Loading');
  }, { timeout: 10000 });

  return tableName;
}

test.describe('Column Operations', () => {
  test('shows existing columns', async ({ page }) => {
    await loginAndCreateTable(page);
    await expect(page.locator('#column-list')).toContainText('id');
    await expect(page.locator('#column-list')).toContainText('INTEGER');
  });

  test('add a new column', async ({ page }) => {
    await loginAndCreateTable(page);
    await page.locator('input[name="column_name"]').fill('email');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();

    await expect(page.locator('#column-list')).toContainText('email', { timeout: 5000 });
    await expect(page.locator('#column-list')).toContainText('TEXT');
  });

  test('remove a column', async ({ page }) => {
    await loginAndCreateTable(page);

    // First add a column to remove
    await page.locator('input[name="column_name"]').fill('temp_col');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('temp_col', { timeout: 5000 });

    // Set up dialog handler BEFORE clicking (use once to avoid stale handlers on retry)
    page.once('dialog', (dialog: any) => dialog.accept());

    // Remove the column — click the Remove button in the row containing 'temp_col'
    const row = page.locator('tr', { has: page.locator('text=temp_col') });
    await Promise.all([
      page.waitForResponse((resp: any) => resp.url().includes('/columns/temp_col') && resp.status() === 200),
      row.locator('button:text("Remove")').click(),
    ]);

    await expect(page.locator('#column-list')).not.toContainText('temp_col', { timeout: 5000 });
  });

  test('add multiple columns of different types', async ({ page }) => {
    await loginAndCreateTable(page);

    // Add TEXT column
    await page.locator('input[name="column_name"]').fill('name');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('name', { timeout: 5000 });

    // Add REAL column
    await page.locator('input[name="column_name"]').fill('price');
    await page.locator('select[name="column_type"]').selectOption('REAL');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('price', { timeout: 5000 });
    await expect(page.locator('#column-list')).toContainText('REAL');
  });
});
