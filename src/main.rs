use serenity::{
    async_trait,
    model::{gateway::Ready, id::ChannelId},
    prelude::*,
};
use reqwest;
use serde::Deserialize;
use std::env;
use tokio::time::{sleep, Duration};
use dotenv::dotenv;
use log::{info, error};
use env_logger;
use tokio::signal;

struct Handler {
    channel_id: ChannelId,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connecté en tant que {}", ready.user.name);
    
        if let Err(e) = self.channel_id.say(&ctx.http, "Bot est prêt et connecté !").await {
            error!("Erreur lors de l'envoi du message de test : {}", e);
        } else {
            info!("Message de test envoyé avec succès.");
        }
    
        // Lancer la tâche de vérification des frais
        tokio::spawn(check_fees(ctx));
    }
}

#[derive(Deserialize)]
struct FeeData {
    fastestFee: u64, // sat/vByte
    halfHourFee: u64,
    hourFee: u64,
}

async fn check_fees(ctx: Context) {
    // Récupérer les variables d'environnement
    let channel_id = env::var("CHANNEL_ID")
        .expect("CHANNEL_ID non définie")
        .parse::<u64>()
        .expect("CHANNEL_ID doit être un nombre");

    let channel_id = ChannelId(channel_id);

    let api_url = "https://mempool.space/api/v1/fees/recommended";
    let fee_threshold = 2;
    let mut last_notified = false;

    loop {
        match reqwest::get(api_url).await {
            Ok(response) => {
                match response.json::<FeeData>().await {
                    Ok(data) => {
                        let current_fee = data.fastestFee;
                        info!("Frais actuels : {} sat/vByte", current_fee);

                        if current_fee <= fee_threshold && !last_notified {
                            if let Err(e) = channel_id.say(&ctx.http, format!(
                                "⚠️ Les frais de transaction Bitcoin sont maintenant à {} sat/vByte!",
                                current_fee
                            )).await {
                                error!("Erreur lors de l'envoi du message : {}", e);
                            } else {
                                last_notified = true;
                                info!("Notification envoyée.");
                            }
                        } else if current_fee > fee_threshold && last_notified {
                            // Réinitialise la notification lorsque les frais remontent au-dessus du seuil
                            last_notified = false;
                            info!("Frais de transaction réinitialisés.");
                        }
                    }
                    Err(e) => {
                        error!("Erreur lors de la désérialisation des données : {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Erreur lors de la requête à l'API : {}", e);
            }
        }

        // Attendre 5 minutes avant la prochaine vérification
        sleep(Duration::from_secs(300)).await;
    }
}

#[tokio::main]
async fn main() {
    // Charger les variables d'environnement depuis le fichier .env
    dotenv().ok();

    // Initialiser le logger
    env_logger::init();
    info!("Bot démarré, en attente de connexion...");

    // Récupérer le token Discord depuis les variables d'environnement
    let token = env::var("DISCORD_TOKEN")
        .expect("DISCORD_TOKEN non définie dans le fichier .env")
        .trim()
        .to_string();
    info!("Token récupéré.");

    // Récupérer l'ID du canal depuis les variables d'environnement
    let channel_id = env::var("CHANNEL_ID")
        .expect("CHANNEL_ID non définie dans le fichier .env")
        .parse::<u64>()
        .expect("CHANNEL_ID doit être un nombre");
    info!("ID du canal récupéré : {}", channel_id);

    let channel_id = ChannelId(channel_id);

    // Définir les intents nécessaires pour recevoir l'événement Ready
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES;

    // Créer le client avec le gestionnaire d'événements
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler { channel_id })
        .await
        .expect("Erreur lors de la création du client");
    info!("Client créé.");

    // Démarrer le client avec gestion gracieuse des arrêts
    tokio::select! {
        res = client.start() => {
            if let Err(why) = res {
                error!("Erreur du client : {:?}", why);
            }
        },
        _ = signal::ctrl_c() => {
            info!("Signal de terminaison reçu, arrêt du bot...");
        },
    }

    info!("Bot arrêté.");
}
