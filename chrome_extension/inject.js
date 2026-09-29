/**
 * DLP Partial Access In-Page Hooks (inject.js)
 * Intercepts file inputs, drag-and-drop, clipboard paste, and fetch/XHR uploads.
 */
(function() {
    'use strict';

    function showDlpBlock(reason) {
        const existing = document.getElementById('dlp-block-overlay');
        if (existing) return;

        const overlay = document.createElement('div');
        overlay.id = 'dlp-block-overlay';
        overlay.style.cssText = `
            position: fixed; top: 0; left: 0; width: 100%; z-index: 9999999;
            background: #dc2626; color: white; padding: 12px 24px;
            font-family: -apple-system, sans-serif; font-size: 14px;
            display: flex; align-items: center; gap: 12px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.3); font-weight: 500;
        `;
        overlay.innerHTML = `
            <span style="font-size:20px">🛡️</span>
            <div><strong>DLP Security Alert:</strong> ${reason} is strictly blocked on this domain.</div>
            <button onclick="this.parentElement.remove()"
                style="margin-left:auto; background:white; color:#dc2626;
                       border:none; padding:6px 14px; border-radius:4px; cursor:pointer; font-weight:bold;">
                Dismiss
            </button>
        `;
        document.body.prepend(overlay);
        setTimeout(() => overlay.remove(), 6000);
    }

    // 1. Block File Input Dialogs
    function attachFileInputBlocker(input) {
        input.addEventListener('click', function(e) {
            e.preventDefault();
            e.stopImmediatePropagation();
            showDlpBlock('File upload dialog');
            return false;
        }, true);

        input.addEventListener('change', function(e) {
            e.preventDefault();
            e.stopImmediatePropagation();
            this.value = '';
            showDlpBlock('File upload');
        }, true);
    }

    // Observe dynamically added file inputs
    const observer = new MutationObserver((mutations) => {
        mutations.forEach(m => {
            m.addedNodes.forEach(node => {
                if (node.nodeType === 1) {
                    node.querySelectorAll('input[type="file"]').forEach(attachFileInputBlocker);
                    if (node.tagName === 'INPUT' && node.type === 'file') {
                        attachFileInputBlocker(node);
                    }
                }
            });
        });
    });

    observer.observe(document.documentElement, { childList: true, subtree: true });
    document.querySelectorAll('input[type="file"]').forEach(attachFileInputBlocker);

    // 2. Block Drag and Drop Files
    ['dragenter', 'dragover', 'drop'].forEach(evt => {
        document.addEventListener(evt, function(e) {
            if (e.dataTransfer && e.dataTransfer.types.includes('Files')) {
                e.preventDefault();
                e.stopImmediatePropagation();
                if (evt === 'drop') {
                    showDlpBlock('File drag and drop');
                }
            }
        }, true);
    });

    // 3. Block File Paste
    document.addEventListener('paste', function(e) {
        const items = e.clipboardData && e.clipboardData.items;
        if (items) {
            for (let item of items) {
                if (item.kind === 'file') {
                    e.preventDefault();
                    e.stopImmediatePropagation();
                    showDlpBlock('File clipboard paste');
                    return;
                }
            }
        }
    }, true);

    // 4. Block Fetch with Files (FormData)
    const origFetch = window.fetch;
    window.fetch = function(url, options) {
        if (options && options.body instanceof FormData) {
            for (let [, val] of options.body.entries()) {
                if (val instanceof File) {
                    showDlpBlock('File upload via network fetch');
                    return Promise.reject(new Error('DLP: Upload Blocked'));
                }
            }
        }
        return origFetch.apply(this, arguments);
    };

    // 5. Block XHR with Files (FormData)
    const origXhrSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function(body) {
        if (body instanceof FormData) {
            for (let [, val] of body.entries()) {
                if (val instanceof File) {
                    showDlpBlock('File upload via XMLHttpRequest');
                    return;
                }
            }
        }
        return origXhrSend.apply(this, arguments);
    };

    console.log('[DLP] In-page Partial Access hooks active.');
})();
