import fs from 'node:fs';
import { chromium } from 'playwright';
const src=fs.readFileSync('capture/capture.mjs','utf8');
const patch=new Function('NOW',src.slice(src.indexOf('const MOCK_PATCH ='),src.indexOf('const themes ='))+'return MOCK_PATCH;')('2026-01-01T00:00:00Z');
const browser=await chromium.launch();const page=await browser.newPage({viewport:{width:1920,height:1080},deviceScaleFactor:2});
page.on('pageerror',e=>console.log('PAGE ERROR',e.message));
await page.addInitScript(()=>{localStorage.setItem('verruckt.theme.v2',JSON.stringify({visualMotif:'buoy',iconMotif:'buoy',colorScheme:'light'}));localStorage.setItem('verruckt.localStart','1')});
const base='http://127.0.0.1:1420/preview.html?fixtures=1';
async function shot(name){await page.screenshot({path:`shots/competition/${name}.png`});console.log(name)}
await page.goto(base+'&view=create');await page.locator('#setup-topic').waitFor();await page.locator('#setup-topic').fill('射影几何');await shot('create-topic');await page.locator('.setup-actions button.primary').click();await page.waitForTimeout(600);await shot('create-level');
await page.goto(base);await page.locator('.stop').first().waitFor();await page.waitForTimeout(1200);await shot('roadmap');await page.evaluate(patch);await page.locator('.stop[data-node-id="n3"] .stop-info').click();await page.locator('button.primary.floating').waitFor();await page.waitForTimeout(500);await shot('lesson');await page.locator('button.primary.floating').click();await page.locator('#chat-input').waitFor();await page.waitForTimeout(900);await shot('opening');
await page.locator('#chat-input').fill('长度保持不变？');await page.locator('#chat-input').press('Enter');await page.waitForTimeout(1800);await shot('probe');
await page.locator('#chat-input').fill('交比？四个点的交比不变。');await page.locator('#chat-input').press('Enter');await page.waitForTimeout(1800);await shot('confirm');
await page.getByRole('button',{name:'阶段检查',exact:true}).click();await page.locator('#quiz .option').nth(1).click();await page.locator('#quiz').getByRole('button',{name:'提交答案'}).click();await page.locator('#quiz .feedback').waitFor();await page.locator('#quiz .feedback').scrollIntoViewIfNeeded();await page.waitForTimeout(700);await shot('quiz-feedback');await browser.close();
