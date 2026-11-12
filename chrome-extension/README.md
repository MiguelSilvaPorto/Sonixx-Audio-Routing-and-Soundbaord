# Sonixx Tab Bridge (extensão do Chrome)

O Windows expõe o Chrome como **uma única sessão de áudio**. Para saber quais
**abas** estão tocando, o Sonixx usa esta extensão local: ela consulta
`chrome.tabs.query({ audible: true })` e envia título/URL para o app em
`http://127.0.0.1:37645/tabs`.

Ela **não captura áudio** e não envia nada para fora da máquina. É só um sensor
de abas.

## Instalar

1. Abra `chrome://extensions`.
2. Ative o **Modo do desenvolvedor**.
3. Clique em **Carregar sem compactação**.
4. Selecione esta pasta `chrome-extension`.
5. Deixe o Sonixx aberto. As abas audíveis aparecem na aba **Chrome** do app.

## Limitações

- A extensão mostra as abas, mas o áudio continua sendo mixado pelo Chrome em
  uma única sessão do Windows. Para capturar/controlar o áudio de cada aba
  separadamente seria necessário usar `chrome.tabCapture` + streaming PCM para
  o app — uma próxima fase.
- O Sonixx precisa estar aberto para a porta 37645 responder.
