import { test, expect } from '@playwright/test';
import { generateToken } from './helpers';

// Use a counter to generate unique table names across tests
let tableCounter = 0;

test.describe('Column Operations', () => {
  test.beforeEach(async ({ page }) => {
    tableCounter++;
    const tableName = `coltest_${tableCounter}`;

    // Store table name on the page for use in tests
    await page.evaluate((name) => {
      (window as any).__tableName = name;
    }, tableName);

    const token = generateToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');
    await page.waitForLoadState('networkidle');

    // Create the test table
    await page.locator('input[name="table_name"]').fill(tableName);
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText(tableName);

    // Wait for htmx swap to settle
    await page.waitForTimeout(500);

    // Navigate to table detail
    await page.locator(`a:text("${tableName}")`).click();
    await expect(page).toHaveURL(new RegExp(`/tables/${tableName}`));

    // Wait for the column list htmx request to complete
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#column-list')).toContainText('id');
  });

  test('shows existing columns', async ({ page }) => {
    await expect(page.locator('#column-list')).toContainText('id');
    await expect(page.locator('#column-list')).toContainText('INTEGER');
  });

  test('add a new column', async ({ page }) => {
    await page.locator('input[name="column_name"]').fill('email');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();

    await expect(page.locator('#column-list')).toContainText('email');
    await expect(page.locator('#column-list')).toContainText('TEXT');
  });

  test('remove a column', async ({ page }) => {
    // First add a column to remove
    await page.locator('input[name="column_name"]').fill('temp_col');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('temp_col');

    // Accept the confirmation dialog
    page.on('dialog', dialog => dialog.accept());

    // Remove the column — click the Remove button in the row containing 'temp_col'
    const row = page.locator('tr', { has: page.locator('text=temp_col') });
    await row.locator('button:text("Remove")').click();

    await expect(page.locator('#column-list')).not.toContainText('temp_col');
  });

  test('add multiple columns of different types', async ({ page }) => {
    // Add TEXT column
    await page.locator('input[name="column_name"]').fill('name');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('name');

    // Add REAL column
    await page.locator('input[name="column_name"]').fill('price');
    await page.locator('select[name="column_type"]').selectOption('REAL');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('price');
    await expect(page.locator('#column-list')).toContainText('REAL');
  });
});
