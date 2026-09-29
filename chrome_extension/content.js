/**
 * DLP Security Extension - content.js (Runs in page context)
 */
(function() {
    "use strict";

    const currentHost = window.location.hostname.toLowerCase().replace(/^www\./, "");
    if (!currentHost) return;

    chrome.storage.local.get(["partialAccessDomains"], (data) => {
        const domains = data.partialAccessDomains || [];
        const isMonitored = domains.some((d) => {
            const p = d.toLowerCase().trim().replace(/^www\./, "");
            return p && (currentHost === p || currentHost.endsWith("." + p));
        });

        if (isMonitored) {
            try {
                const script = document.createElement("script");
                script.src = chrome.runtime.getURL("inject.js");
                script.setAttribute("data-dlp", "true");
                (document.head || document.documentElement).appendChild(script);
                script.remove();
            } catch (e) {
                console.error("[DLP] Hook injection failed:", e);
            }
        }
    });
})();
