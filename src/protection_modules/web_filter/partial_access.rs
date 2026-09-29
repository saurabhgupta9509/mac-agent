//! protection_modules/web_filter/partial_access.rs
//!
//! ╔══════════════════════════════════════════════════════════════════╗
//! ║     PARTIAL ACCESS — Upload/Download Blocking (macOS)           ║
//! ║                                                                  ║
//! ║  HOW IT WORKS:                                                   ║
//! ║  ┌───────────────────────────────────────────────────────┐      ║
//! ║  │  1. MITM proxy intercepts HTTP response from site     │      ║
//! ║  │  2. Checks if site is in partial_access policy list   │      ║
//! ║  │  3. Injects <script> into HTML response body          │      ║
//! ║  │  4. Injected JS:                                       │      ║
//! ║  │     - Monitors all <input type="file"> elements        │      ║
//! ║  │     - Intercepts drag-and-drop events                  │      ║
//! ║  │     - Intercepts paste events with file content        │      ║
//! ║  │     - Blocks fetch/XHR with multipart/form-data       │      ║
//! ║  │     - Shows DLP warning overlay to user               │      ║
//! ║  │  5. Download blocking:                                 │      ║
//! ║  │     - Intercepts Content-Disposition: attachment       │      ║
//! ║  │     - Returns 403 instead of file                      │      ║
//! ║  └───────────────────────────────────────────────────────┘      ║
//! ║                                                                  ║
//! ║  Chrome Extension also provides secondary blocking:             ║
//! ║  - content.js: intercepts DOM events in Chrome                  ║
//! ║  - background.js: blocks webRequest with file content           ║
//! ║                                                                  ║
//! ║  Same JS injection as Linux agent — fully portable!             ║
//! ╚══════════════════════════════════════════════════════════════════╝

/// The JavaScript injected into partial-access sites.
/// Identical to linux agent's inject.js — works on macOS Chrome/Safari/Firefox.
pub const PARTIAL_ACCESS_INJECT_JS: &str = r#"
(function() {
    'use strict';

    // ── Configuration sent by DLP proxy ────────────────────────────────────
    const DLP_CONFIG = {
        blockUploads: true,
        blockDownloads: true,
        showWarning: true,
        siteName: window.location.hostname,
    };

    // ── Warning overlay ────────────────────────────────────────────────────
    function showDlpBlock(reason) {
        const existing = document.getElementById('dlp-block-overlay');
        if (existing) return;

        const overlay = document.createElement('div');
        overlay.id = 'dlp-block-overlay';
        overlay.style.cssText = `
            position: fixed; top: 0; left: 0; width: 100%; z-index: 999999;
            background: #ff4444; color: white; padding: 12px 20px;
            font-family: -apple-system, sans-serif; font-size: 14px;
            display: flex; align-items: center; gap: 10px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.3);
        `;
        overlay.innerHTML = `
            <span style="font-size:18px">🛡️</span>
            <strong>DLP Policy:</strong> ${reason} is blocked on this site.
            <button onclick="this.parentElement.remove()"
                style="margin-left:auto; background:white; color:#ff4444;
                       border:none; padding:4px 12px; border-radius:4px; cursor:pointer;">
                Dismiss
            </button>
        `;
        document.body.prepend(overlay);
        setTimeout(() => overlay.remove(), 5000);
    }

    // ── Block file uploads ─────────────────────────────────────────────────
    function blockFileInput(input) {
        input.addEventListener('click', function(e) {
            e.preventDefault();
            e.stopImmediatePropagation();
            showDlpBlock('File upload');
            return false;
        }, true);

        input.addEventListener('change', function(e) {
            e.preventDefault();
            e.stopImmediatePropagation();
            this.value = '';
            showDlpBlock('File upload');
        }, true);
    }

    // Observe DOM for dynamically added file inputs
    const observer = new MutationObserver((mutations) => {
        mutations.forEach(m => {
            m.addedNodes.forEach(node => {
                if (node.nodeType === 1) {
                    node.querySelectorAll('input[type="file"]').forEach(blockFileInput);
                    if (node.tagName === 'INPUT' && node.type === 'file') {
                        blockFileInput(node);
                    }
                }
            });
        });
    });

    observer.observe(document.documentElement, { childList: true, subtree: true });
    document.querySelectorAll('input[type="file"]').forEach(blockFileInput);

    // ── Block drag-and-drop ────────────────────────────────────────────────
    ['dragenter', 'dragover', 'drop'].forEach(evt => {
        document.addEventListener(evt, function(e) {
            if (e.dataTransfer && e.dataTransfer.types.includes('Files')) {
                e.preventDefault();
                e.stopImmediatePropagation();
                if (evt === 'drop') { showDlpBlock('File drag & drop'); }
            }
        }, true);
    });

    // ── Block paste with files ─────────────────────────────────────────────
    document.addEventListener('paste', function(e) {
        const items = e.clipboardData && e.clipboardData.items;
        if (items) {
            for (let item of items) {
                if (item.kind === 'file') {
                    e.preventDefault();
                    e.stopImmediatePropagation();
                    showDlpBlock('File paste');
                    return;
                }
            }
        }
    }, true);

    // ── Block XHR/fetch with file payload ─────────────────────────────────
    const origFetch = window.fetch;
    window.fetch = function(url, options) {
        if (options && options.body instanceof FormData) {
            for (let [, val] of options.body.entries()) {
                if (val instanceof File) {
                    showDlpBlock('File upload (fetch)');
                    return Promise.reject(new Error('DLP: File upload blocked'));
                }
            }
        }
        return origFetch.apply(this, arguments);
    };

    const origXhrSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function(body) {
        if (body instanceof FormData) {
            for (let [, val] of body.entries()) {
                if (val instanceof File) {
                    showDlpBlock('File upload (XHR)');
                    return;
                }
            }
        }
        return origXhrSend.apply(this, arguments);
    };

    console.log('[DLP] Partial access protection active on:', window.location.hostname);
})();
"#;
