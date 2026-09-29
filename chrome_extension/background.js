/**
 * DLP Security Extension - background.js (macOS Service Worker)
 *
 * Responsibilities:
 * 1. URL Blocking with Instant Tab Closing or Redirect to blocked.html.
 * 2. Native Messaging Host connection to root daemon.
 * 3. Partial Access (Upload/Download intent relaying).
 */

const HOST_NAME = "com.dlp.agent";
let port = null;
let blockedDomains = [];
let partialAccessDomains = [];

// ── 1. Native Messaging Connection to dlp-agent daemon ──────────────
function connectNative() {
    try {
        port = chrome.runtime.connectNative(HOST_NAME);

        port.onMessage.addListener((msg) => {
            if (msg.type === "blocklist_update" && Array.isArray(msg.domains)) {
                blockedDomains = msg.domains;
                chrome.storage.local.set({ blockedDomains });
                enforceBlocklistOnAllTabs();
            } else if (msg.type === "partial_access_update" && Array.isArray(msg.domains)) {
                partialAccessDomains = msg.domains;
                chrome.storage.local.set({ partialAccessDomains });
            } else if (msg.action === "close_tab" && msg.tabId) {
                chrome.tabs.remove(msg.tabId).catch(() => {});
            }
        });

        port.onDisconnect.addListener(() => {
            port = null;
            setTimeout(connectNative, 5000);
        });

        // Request initial active policies
        port.postMessage({ type: "get_policy" });
    } catch (e) {
        setTimeout(connectNative, 5000);
    }
}

connectNative();

// ── 2. Real-Time Tab Blocking & Instant Closing / Redirection ────────
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
    if (changeInfo.url) {
        checkAndEnforceTab(tabId, changeInfo.url);
    }
});

chrome.tabs.onActivated.addListener((activeInfo) => {
    chrome.tabs.get(activeInfo.tabId, (tab) => {
        if (tab && tab.url) {
            checkAndEnforceTab(tab.id, tab.url);
        }
    });
});

function checkAndEnforceTab(tabId, url) {
    if (!url || url.startsWith("chrome://") || url.startsWith("chrome-extension://")) {
        return;
    }

    try {
        const parsed = new URL(url);
        const host = parsed.hostname.toLowerCase().replace(/^www\./, "");

        for (const blocked of blockedDomains) {
            const b = blocked.toLowerCase().trim().replace(/^www\./, "");
            if (b && (host === b || host.endsWith("." + b))) {
                console.warn(`[DLP] Blocked domain accessed: ${host}. Enforcing Tab Action...`);

                // Option 1: Redirect to local DLP block page
                const blockPageUrl = chrome.runtime.getURL("blocked.html") + "?url=" + encodeURIComponent(url);
                chrome.tabs.update(tabId, { url: blockPageUrl }).catch(() => {
                    // Option 2 (Fallback): Instantly close tab
                    chrome.tabs.remove(tabId).catch(() => {});
                });
                break;
            }
        }
    } catch (_) {}
}

function enforceBlocklistOnAllTabs() {
    chrome.tabs.query({}, (tabs) => {
        for (const tab of tabs) {
            if (tab.id && tab.url) {
                checkAndEnforceTab(tab.id, tab.url);
            }
        }
    });
}
