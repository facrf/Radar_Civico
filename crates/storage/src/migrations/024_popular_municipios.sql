-- Migration 024: Popular todos os municípios do Espírito Santo e principais cidades brasileiras
-- Permite filtragem geográfica rica por município em todos os estados, corrigindo a limitação
-- onde apenas a capital Vitória era listada no Espírito Santo.

INSERT OR IGNORE INTO municipios_ibge (codigo_ibge, nome, uf, latitude, longitude, is_capital) VALUES
-- Todos os 78 municípios do Espírito Santo (ES)
(3200102, 'Afonso Cláudio', 'ES', -20.0753, -41.1378, 0),
(3200169, 'Água Doce do Norte', 'ES', -18.5478, -40.9856, 0),
(3200136, 'Águia Branca', 'ES', -18.9839, -40.7403, 0),
(3200201, 'Alegre', 'ES', -20.7631, -41.5331, 0),
(3200300, 'Alfredo Chaves', 'ES', -20.6358, -40.7511, 0),
(3200359, 'Alto Rio Novo', 'ES', -19.0578, -41.0181, 0),
(3200409, 'Anchieta', 'ES', -20.8058, -40.6453, 0),
(3200508, 'Apiacá', 'ES', -21.1547, -41.5678, 0),
(3200607, 'Aracruz', 'ES', -19.8203, -40.2733, 0),
(3200706, 'Atílio Vivácqua', 'ES', -20.9142, -41.1981, 0),
(3200805, 'Baixo Guandu', 'ES', -19.5189, -41.0153, 0),
(3200904, 'Barra de São Francisco', 'ES', -18.7553, -40.8911, 0),
(3201001, 'Boa Esperança', 'ES', -18.5403, -40.2958, 0),
(3201100, 'Bom Jesus do Norte', 'ES', -21.1897, -41.6708, 0),
(3201159, 'Brejetuba', 'ES', -20.1436, -41.2936, 0),
(3201209, 'Cachoeiro de Itapemirim', 'ES', -20.8489, -41.1128, 0),
(3201308, 'Cariacica', 'ES', -20.2639, -40.4200, 0),
(3201407, 'Castelo', 'ES', -20.6036, -41.2039, 0),
(3201506, 'Colatina', 'ES', -19.5392, -40.6308, 0),
(3201605, 'Conceição da Barra', 'ES', -18.5933, -39.7322, 0),
(3201704, 'Conceição do Castelo', 'ES', -20.3689, -41.2439, 0),
(3201803, 'Divino de São Lourenço', 'ES', -20.6200, -41.6853, 0),
(3201902, 'Domingos Martins', 'ES', -20.3633, -40.6589, 0),
(3202009, 'Dores do Rio Preto', 'ES', -20.6886, -41.8456, 0),
(3202108, 'Ecoporanga', 'ES', -18.3736, -40.8306, 0),
(3202207, 'Fundão', 'ES', -19.9333, -40.4042, 0),
(3202256, 'Governador Lindenberg', 'ES', -19.2778, -40.4789, 0),
(3202306, 'Guaçuí', 'ES', -20.7758, -41.6792, 0),
(3202405, 'Guarapari', 'ES', -20.6700, -40.4975, 0),
(3202454, 'Ibatiba', 'ES', -20.2339, -41.5103, 0),
(3202504, 'Ibiraçu', 'ES', -19.8322, -40.3703, 0),
(3202553, 'Ibitirama', 'ES', -20.5414, -41.6681, 0),
(3202603, 'Iconha', 'ES', -20.7931, -40.8106, 0),
(3202652, 'Irupi', 'ES', -20.3447, -41.6406, 0),
(3202702, 'Itaguaçu', 'ES', -19.8025, -40.8558, 0),
(3202801, 'Itapemirim', 'ES', -21.0111, -40.8339, 0),
(3202900, 'Itarana', 'ES', -19.8739, -40.8753, 0),
(3203007, 'Iúna', 'ES', -20.3458, -41.5358, 0),
(3203056, 'Jaguaré', 'ES', -18.9056, -40.0761, 0),
(3203106, 'Jerônimo Monteiro', 'ES', -20.7892, -41.3947, 0),
(3203130, 'João Neiva', 'ES', -19.7578, -40.3831, 0),
(3203163, 'Laranja da Terra', 'ES', -19.8986, -41.0558, 0),
(3203205, 'Linhares', 'ES', -19.3911, -40.0722, 0),
(3203304, 'Mantenópolis', 'ES', -18.8631, -41.1228, 0),
(3203320, 'Marataízes', 'ES', -21.0433, -40.8244, 0),
(3203346, 'Marechal Floriano', 'ES', -20.4131, -40.6831, 0),
(3203353, 'Marilândia', 'ES', -19.4136, -40.5422, 0),
(3203403, 'Mimoso do Sul', 'ES', -21.0642, -41.3653, 0),
(3203502, 'Montanha', 'ES', -18.1269, -40.3633, 0),
(3203601, 'Mucurici', 'ES', -18.0933, -40.5158, 0),
(3203700, 'Muniz Freire', 'ES', -20.4642, -41.4131, 0),
(3203809, 'Muqui', 'ES', -20.9519, -41.3458, 0),
(3203908, 'Nova Venécia', 'ES', -18.7106, -40.4006, 0),
(3204005, 'Pancas', 'ES', -19.2250, -40.8514, 0),
(3204054, 'Pedro Canário', 'ES', -18.0294, -40.1494, 0),
(3204104, 'Pinheiros', 'ES', -18.4061, -40.2144, 0),
(3204203, 'Piúma', 'ES', -20.8347, -40.7289, 0),
(3204252, 'Ponto Belo', 'ES', -18.1247, -40.5408, 0),
(3204302, 'Presidente Kennedy', 'ES', -21.0964, -41.0486, 0),
(3204351, 'Rio Bananal', 'ES', -19.2647, -40.3331, 0),
(3204401, 'Rio Novo do Sul', 'ES', -20.8592, -40.9356, 0),
(3204500, 'Santa Leopoldina', 'ES', -20.1006, -40.5297, 0),
(3204559, 'Santa Maria de Jetibá', 'ES', -20.0406, -40.7461, 0),
(3204609, 'Santa Teresa', 'ES', -19.9358, -40.5997, 0),
(3204658, 'São Domingos do Norte', 'ES', -19.1417, -40.6272, 0),
(3204708, 'São Gabriel da Palha', 'ES', -19.0169, -40.5361, 0),
(3204807, 'São José do Calçado', 'ES', -20.9817, -41.6547, 0),
(3204906, 'São Mateus', 'ES', -18.7161, -39.8589, 0),
(3204955, 'São Roque do Canaã', 'ES', -19.7389, -40.6567, 0),
(3205002, 'Serra', 'ES', -20.1286, -40.3078, 0),
(3205010, 'Sooretama', 'ES', -19.1969, -40.0931, 0),
(3205036, 'Vargem Alta', 'ES', -20.6728, -41.0081, 0),
(3205069, 'Venda Nova do Imigrante', 'ES', -20.3253, -41.1342, 0),
(3205101, 'Viana', 'ES', -20.3906, -40.4958, 0),
(3205150, 'Vila Pavão', 'ES', -18.6144, -40.6106, 0),
(3205176, 'Vila Valério', 'ES', -18.9986, -40.3892, 0),
(3205200, 'Vila Velha', 'ES', -20.3297, -40.2925, 0),

-- Principais municípios de São Paulo (SP)
(3518800, 'Guarulhos', 'SP', -23.4538, -46.5333, 0),
(3548708, 'São Bernardo do Campo', 'SP', -23.6939, -46.5650, 0),
(3547809, 'Santo André', 'SP', -23.6639, -46.5383, 0),
(3534401, 'Osasco', 'SP', -23.5325, -46.7917, 0),
(3552205, 'Sorocaba', 'SP', -23.5017, -47.4581, 0),
(3549805, 'São José do Rio Preto', 'SP', -20.8114, -49.3758, 0),
(3525904, 'Jundiaí', 'SP', -23.1864, -46.8842, 0),
(3538709, 'Piracicaba', 'SP', -22.7253, -47.6492, 0),
(3506003, 'Bauru', 'SP', -22.3147, -49.0606, 0),
(3516200, 'Franca', 'SP', -20.5386, -47.4008, 0),
(3554102, 'Taubaté', 'SP', -23.0264, -45.5553, 0),
(3541000, 'Praia Grande', 'SP', -24.0058, -46.4028, 0),
(3505708, 'Barueri', 'SP', -23.5111, -46.8764, 0),
(3526902, 'Limeira', 'SP', -22.5647, -47.4017, 0),
(3520509, 'Indaiatuba', 'SP', -23.0903, -47.2181, 0),
(3501608, 'Americana', 'SP', -22.7394, -47.3314, 0),
(3503208, 'Araraquara', 'SP', -21.7944, -48.1758, 0),

-- Principais municípios do Rio de Janeiro (RJ)
(3304904, 'São Gonçalo', 'RJ', -22.8269, -43.0539, 0),
(3301702, 'Duque de Caxias', 'RJ', -22.7856, -43.3117, 0),
(3303500, 'Nova Iguaçu', 'RJ', -22.7556, -43.4603, 0),
(3300456, 'Belford Roxo', 'RJ', -22.7639, -43.3997, 0),
(3301009, 'Campos dos Goytacazes', 'RJ', -21.7539, -41.3239, 0),
(3305109, 'Volta Redonda', 'RJ', -22.5231, -44.1042, 0),
(3303906, 'Petrópolis', 'RJ', -22.5050, -43.1789, 0),
(3302403, 'Macaé', 'RJ', -22.3769, -41.7869, 0),
(3300704, 'Cabo Frio', 'RJ', -22.8794, -42.0186, 0),
(3300100, 'Angra dos Reis', 'RJ', -23.0067, -44.3181, 0),

-- Principais municípios de Minas Gerais (MG)
(3118601, 'Contagem', 'MG', -19.9317, -44.0536, 0),
(3106705, 'Betim', 'MG', -19.9678, -44.1983, 0),
(3143302, 'Montes Claros', 'MG', -16.7281, -43.8617, 0),
(3170107, 'Uberaba', 'MG', -19.7472, -47.9319, 0),
(3127701, 'Governador Valadares', 'MG', -18.8511, -41.9494, 0),
(3131307, 'Ipatinga', 'MG', -19.4683, -42.5367, 0),
(3167202, 'Sete Lagoas', 'MG', -19.4597, -44.2472, 0),
(3122306, 'Divinópolis', 'MG', -20.1458, -44.8919, 0),
(3151800, 'Poços de Caldas', 'MG', -21.7878, -46.5614, 0),
(3148004, 'Patos de Minas', 'MG', -18.5789, -46.5181, 0),
(3152501, 'Pouso Alegre', 'MG', -22.2300, -45.9364, 0),
(3169307, 'Varginha', 'MG', -21.5517, -45.4300, 0),

-- Principais municípios da Bahia (BA)
(2933307, 'Vitória da Conquista', 'BA', -14.8661, -40.8394, 0),
(2905701, 'Camaçari', 'BA', -12.6975, -38.3242, 0),
(2918407, 'Juazeiro', 'BA', -9.4167, -40.5000, 0),
(2914802, 'Itabuna', 'BA', -14.7939, -39.2800, 0),
(2919207, 'Lauro de Freitas', 'BA', -12.8944, -38.3272, 0),
(2913606, 'Ilhéus', 'BA', -14.7889, -39.0494, 0),
(2918001, 'Jequié', 'BA', -13.8581, -40.0842, 0),
(2931350, 'Teixeira de Freitas', 'BA', -16.5469, -39.7397, 0),
(2900702, 'Alagoinhas', 'BA', -12.1356, -38.4192, 0),
(2903201, 'Barreiras', 'BA', -12.1528, -44.9961, 0),
(2925303, 'Porto Seguro', 'BA', -16.4497, -39.0647, 0),

-- Principais municípios do Paraná (PR)
(4119905, 'Ponta Grossa', 'PR', -25.0950, -50.1619, 0),
(4104808, 'Cascavel', 'PR', -24.9558, -53.4553, 0),
(4125506, 'São José dos Pinhais', 'PR', -25.5347, -49.2064, 0),
(4108304, 'Foz do Iguaçu', 'PR', -25.5478, -54.5881, 0),
(4105805, 'Colombo', 'PR', -25.2917, -49.2242, 0),
(4109401, 'Guarapuava', 'PR', -25.3953, -51.4581, 0),
(4118204, 'Paranaguá', 'PR', -25.5206, -48.5092, 0),
(4127700, 'Toledo', 'PR', -24.7139, -53.7431, 0),

-- Principais municípios de Santa Catarina (SC)
(4216602, 'São José', 'SC', -27.6136, -48.6367, 0),
(4204202, 'Chapecó', 'SC', -27.1006, -52.6153, 0),
(4208203, 'Itajaí', 'SC', -26.9078, -48.6619, 0),
(4204608, 'Criciúma', 'SC', -28.6775, -49.3703, 0),
(4208906, 'Jaraguá do Sul', 'SC', -26.4850, -49.0664, 0),
(4211900, 'Palhoça', 'SC', -27.6450, -48.6678, 0),
(4209300, 'Lages', 'SC', -27.8161, -50.3261, 0),
(4202008, 'Balneário Camboriú', 'SC', -26.9928, -48.6353, 0),

-- Principais municípios de Goiás (GO)
(5201405, 'Aparecida de Goiânia', 'GO', -16.8236, -49.2439, 0),
(5218805, 'Rio Verde', 'GO', -17.7928, -50.9192, 0),
(5200258, 'Águas Lindas de Goiás', 'GO', -15.7622, -48.2814, 0),
(5212501, 'Luziânia', 'GO', -16.2525, -47.9500, 0),
(5221858, 'Valparaíso de Goiás', 'GO', -16.0689, -47.9761, 0),

-- Principais municípios de Pernambuco (PE)
(2607901, 'Jaboatão dos Guararapes', 'PE', -8.1128, -35.0150, 0),
(2609600, 'Olinda', 'PE', -8.0089, -34.8553, 0),
(2604106, 'Caruaru', 'PE', -8.2833, -35.9761, 0),
(2611101, 'Petrolina', 'PE', -9.3986, -40.5008, 0),
(2610707, 'Paulista', 'PE', -7.9408, -34.8728, 0),

-- Principais municípios do Ceará (CE)
(2303709, 'Caucaia', 'CE', -3.7317, -38.6531, 0),
(2307304, 'Juazeiro do Norte', 'CE', -7.2028, -39.3150, 0),
(2307650, 'Maracanaú', 'CE', -3.8767, -38.6256, 0),
(2312908, 'Sobral', 'CE', -3.6894, -40.3481, 0);

-- Vínculo de municípios para candidaturas do Espírito Santo (onde o campo estava em branco)
UPDATE candidaturas SET municipio = 'Vitória' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo IN (
        'LORENZO SILVA DE PAZOLINI', 'JOÃO CARLOS COSER', 'KARLA SILVA COSER', 'LUIZ PAULO VELLOZO LUCAS',
        'LUCINIO CASTELO DE ASSUMÇÃO', 'CAMILA VALADÃO', 'CAMILA VALADAO', 'DU CARLOS', 'DUVAL CARLOS DE FREITAS'
    ) OR nome_urna IN ('LORENZO PAZOLINI', 'JOÃO COSER', 'KARLA  COSER', 'LUIZ PAULO', 'CAPITÃO ASSUMÇÃO', 'CAMILA VALADÃO')
);

UPDATE candidaturas SET municipio = 'Vila Velha' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%ARNALDO BORGO%' OR nome_urna LIKE '%ARNALDINHO BORGO%'
    OR nome_completo LIKE '%ALEXANDRE RAMALHO%' OR nome_urna LIKE '%CORONEL RAMALHO%'
);

UPDATE candidaturas SET municipio = 'Serra' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%WEVERSON MEIRELES%' OR nome_urna LIKE '%WEVERSON%'
    OR nome_completo LIKE '%PABLO MURIBECA%' OR nome_urna LIKE '%PABLO MURIBECA%'
    OR nome_completo LIKE '%AUDIFAX%'
);

UPDATE candidaturas SET municipio = 'Cariacica' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%EUCLERIO%SAMPAIO%' OR nome_urna LIKE '%EUCLERIO%' OR nome_urna LIKE '%EUCLÉRIO%'
    OR nome_completo LIKE '%CELIA%TAVARES%'
);

UPDATE candidaturas SET municipio = 'Cachoeiro de Itapemirim' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%THEODORICO DE ASSIS FERRACO%' OR nome_urna LIKE '%FERRAÇO%'
    OR nome_completo LIKE '%DIEGO LIBARDI%'
);

UPDATE candidaturas SET municipio = 'Linhares' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%LUCAS SCARAMUSSA%' OR nome_completo LIKE '%BRUNO MARIANELLI%'
);

UPDATE candidaturas SET municipio = 'Colatina' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%RENZO VASCONCELOS%' OR nome_completo LIKE '%GUERINO BALESTRASSI%'
);

UPDATE candidaturas SET municipio = 'Guarapari' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%RODRIGO BORGES%' OR (nome_completo LIKE '%JOSE DE OLIVEIRA%' AND nome_urna LIKE '%ZÉ PRETO%')
);

UPDATE candidaturas SET municipio = 'São Mateus' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%MARCUS SANTOS%' OR nome_completo LIKE '%CARLINDOS ROCHA%'
);

UPDATE candidaturas SET municipio = 'Aracruz' WHERE (municipio IS NULL OR municipio = '') AND uf = 'ES' AND politico_id IN (
    SELECT id FROM politicos WHERE nome_completo LIKE '%LUIZ CARLOS COUTINHO%' OR nome_urna LIKE '%DR COUTINHO%'
);
