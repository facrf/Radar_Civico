"""Atualiza retratos de identidades explícitas, com fonte e licença do Commons.
Executar manualmente: python3 scripts/atualizar_fotos_autoridades.py
Não associa resultados de busca ou nomes parciais a pessoas.
"""
import concurrent.futures, hashlib, json, re, urllib.parse, time
from urllib3.util.retry import Retry
from requests.adapters import HTTPAdapter
from pathlib import Path
import requests

ROOT = Path(__file__).resolve().parents[1]
NAMES = {
'LUÍS ROBERTO BARROSO':'Luís Roberto Barroso','LUIZ EDSON FACHIN':'Edson Fachin',
'GILMAR FERREIRA MENDES':'Gilmar Mendes','CÁRMEN LÚCIA ANTUNES ROCHA':'Cármen Lúcia',
'JOSÉ ANTONIO DIAS TOFFOLI':'Dias Toffoli','LUIZ FUX':'Luiz Fux','ALEXANDRE DE MORAES':'Alexandre de Moraes',
'KASSIO NUNES MARQUES':'Nunes Marques','ANDRÉ LUIZ DE ALMEIDA MENDONÇA':'André Mendonça',
'CRISTIANO ZANIN MARTINS':'Cristiano Zanin','FLÁVIO DINO DE CASTRO E COSTA':'Flávio Dino',
'BRUNO DANTAS NASCIMENTO':'Bruno Dantas','VITAL DO RÊGO FILHO':'Vital do Rêgo Filho',
'WALTON ALENCAR RODRIGUES':'Walton Alencar Rodrigues','BENJAMIN ZYMLER':'Benjamin Zymler',
'JOÃO AUGUSTO RIBEIRO NARDES':'Augusto Nardes','AROLDO CEDRAZ DE OLIVEIRA':'Aroldo Cedraz',
'ANTONIO AUGUSTO JUNHO ANASTASIA':'Antonio Anastasia','JORGE ANTONIO DE OLIVEIRA FRANCISCO':'Jorge Oliveira',
'JHONATAN PEREIRA DE JESUS':'Jhonatan de Jesus','LUIZ INÁCIO LULA DA SILVA':'Luiz Inácio Lula da Silva',
'JAIR MESSIAS BOLSONARO':'Jair Bolsonaro','DILMA VANA ROUSSEFF':'Dilma Rousseff',
'MICHEL MIGUEL ELIAS TEMER LULIA':'Michel Temer','FERNANDO HENRIQUE CARDOSO':'Fernando Henrique Cardoso',
'PAULO GUSTAVO GONET BRANCO':'Paulo Gonet',
}

def load(item):
    name,title=item
    time.sleep(3)
    session=requests.Session();session.mount('https://', HTTPAdapter(max_retries=Retry(total=5, backoff_factor=3, status_forcelist=[429,500,502,503], respect_retry_after_header=True)));session.headers['User-Agent']='RadarCivico/1.0 (public civic data; photo attribution)'
    page=session.get('https://pt.wikipedia.org/api/rest_v1/page/summary/'+urllib.parse.quote(title),timeout=25)
    page.raise_for_status(); summary=page.json()
    original=summary['originalimage']['source']
    filename=urllib.parse.unquote(urllib.parse.urlparse(original).path.rsplit('/',1)[-1])
    if name == 'JORGE ANTONIO DE OLIVEIRA FRANCISCO': filename = 'Foto ministro Jorge.jpg'
    metadata=session.get('https://commons.wikimedia.org/w/api.php',params={'action':'query','format':'json','titles':'File:'+filename,'prop':'imageinfo','iiprop':'url|extmetadata','iiurlwidth':320},timeout=25)
    metadata.raise_for_status(); info=next(iter(metadata.json()['query']['pages'].values()))['imageinfo'][0]
    ext=info['extmetadata']
    image=session.get(info.get('thumburl',info['url']),timeout=30);image.raise_for_status()
    data=image.content
    if not (data.startswith(b'\xff\xd8\xff') or data.startswith(b'\x89PNG\r\n\x1a\n')):raise ValueError('Resposta não é imagem')
    suffix='jpg' if data.startswith(b'\xff\xd8') else 'png'
    asset=hashlib.sha256(name.encode()).hexdigest()[:16]+'.'+suffix
    target=ROOT/'web/static/autoridades'/asset;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
    plain=lambda key: re.sub('<[^>]+>','',ext.get(key,{}).get('value','')).strip()
    print(name, len(data), flush=True)
    return {'nome':name,'arquivo':asset,'url':'/autoridades/'+asset,'fonte':info['descriptionurl'],'autor':plain('Artist'),'licenca':plain('LicenseShortName'),'licenca_url':plain('LicenseUrl')}

if __name__=='__main__':
    records=[]; failures=[]
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        existing=json.loads((ROOT/'web/static/autoridades/fontes.json').read_text()) if (ROOT/'web/static/autoridades/fontes.json').exists() else []
        done={r['nome'] for r in existing}
        futures={pool.submit(load,item):item for item in NAMES.items() if item[0] not in done}
        for future,item in futures.items():
            try:records.append(future.result())
            except Exception as e:failures.append((item[0],str(e)))
    path=ROOT/'web/static/autoridades/fontes.json'
    if path.exists():
        previous=json.loads(path.read_text());new={r['nome']:r for r in previous};new.update({r['nome']:r for r in records});records=list(new.values())
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(records,ensure_ascii=False,indent=2)+'\n')
    print('Falhas:',failures)
    if failures:raise SystemExit(1)
