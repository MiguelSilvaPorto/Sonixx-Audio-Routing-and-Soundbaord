const ENDPOINT = "http://127.0.0.1:37645/tabs";

async function sendTabs() {
  try {
    const tabs = await chrome.tabs.query({ audible: true });
    const payload = {
      tabs: tabs.map((tab) => ({
        id: tab.id ?? 0,
        title: tab.title ?? "",
        url: tab.url ?? "",
        audible: Boolean(tab.audible),
      })),
    };
    await fetch(ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Sonixx-Bridge": "1",
      },
      body: JSON.stringify(payload),
    });
  } catch (error) {
    console.warn("[Sonixx] bridge indisponível:", error);
  }
}

chrome.tabs.onUpdated.addListener((_tabId, changeInfo) => {
  if ("audible" in changeInfo || "title" in changeInfo || "url" in changeInfo) {
    sendTabs();
  }
});
chrome.tabs.onRemoved.addListener(sendTabs);
chrome.runtime.onStartup.addListener(sendTabs);
chrome.runtime.onInstalled.addListener(sendTabs);
setInterval(sendTabs, 3000);
sendTabs();
