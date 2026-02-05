import { test, expect } from '@playwright/test';
import { generateToken } from './helpers';

test.describe('Table Operations', () => {
  test.beforeEach(async ({ page }) => {
    const token = generateToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');
    // Wait for htmx to load the table list
    await page.waitForLoadState('networkidle');
  });

  test('shows empty state when no tables exist', async ({ page }) => {
    await expect(page.locator('#table-list')).toContainText('No tables yet');
  });

  test('create a new table', async ({ page }) => {
    await page.locator('input[name="table_name"]').fill('users');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();

    await expect(page.locator('#table-list')).toContainText('users');
  });

  test('drop a table', async ({ page }) => {
    // First create a table
    await page.locator('input[name="table_name"]').fill('temp_table');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('temp_table');

    // Accept the confirmation dialog
    page.on('dialog', dialog => dialog.accept());

    // Drop the specific table - target the li containing 'temp_table'
    const tableItem = page.locator('li', { has: page.locator('a:text("temp_table")') });
    await tableItem.locator('button:text("Drop")').click();
    await expect(page.locator('#table-list')).not.toContainText('temp_table');
  });

  test('navigate to table detail', async ({ page }) => {
    // Create a table first
    await page.locator('input[name="table_name"]').fill('products');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('products');

    // Wait for htmx swap to settle before clicking
    await page.waitForTimeout(500);

    // Click on the table name
    await page.locator('a:text("products")').click();
    await expect(page).toHaveURL(/\/tables\/products/);
    await expect(page.locator('h1')).toContainText('products');
  });
});
