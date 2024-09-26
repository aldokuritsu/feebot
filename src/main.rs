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
    fee_threshold: u64,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connecté en tant que {}", ready.user.name);

        // Envoyer le message initial
        if let Err(e) = self.channel_id.say(&ctx.http, "Bot est prêt et connecté !").await {
            error!("Erreur lors de l'envoi du message de test : {}", e);
        } else {
            info!("Message de test envoyé avec succès.");
        }

        // Récupérer les frais actuels et envoyer un message d'information
        match fetch_fee_data().await {
            Ok(data) => {
                let _current_fee = data.fastest_fee;
                let message = format!(
                    "🔍 **Frais de Transaction Actuels**\n\
                    - Fastest Fee: {} sat/vByte\n\
                    - Half Hour Fee: {} sat/vByte\n\
                    - Hour Fee: {} sat/vByte\n\n\
                    ⚠️ **Alerte activée pour les frais ≤ {} sat/vByte**",
                    data.fastest_fee, data.half_hour_fee, data.hour_fee, self.fee_threshold
                );

                if let Err(e) = self.channel_id.say(&ctx.http, message).await {
                    error!("Erreur lors de l'envoi du message des frais actuels : {}", e);
                } else {
                    info!("Message des frais actuels envoyé avec succès.");
                }
            }
            Err(e) => {
                error!("Erreur lors de la récupération des frais actuels : {}", e);
                if let Err(e) = self.channel_id.say(&ctx.http, "⚠️ Impossible de récupérer les frais actuels.").await {
                    error!("Erreur lors de l'envoi du message d'erreur : {}", e);
                }
            }
        }

        // Lancer la tâche de vérification des frais toutes les 10 minutes
        tokio::spawn(check_fees(ctx, self.channel_id, self.fee_threshold));
    }
}

#[derive(Deserialize)]
struct FeeData {
    #[serde(rename = "fastestFee")]
    fastest_fee: u64,   // sat/vByte
    
    #[serde(rename = "halfHourFee")]
    half_hour_fee: u64,
    
    #[serde(rename = "hourFee")]
    hour_fee: u64,
}

async fn fetch_fee_data() -> Result<FeeData, reqwest::Error> {
    let _api_url = "https://mempool.space/api/v1/fees/recommended";
    let response = reqwest::get(_api_url).await?;
    let data = response.json::<FeeData>().await?;
    Ok(data)
}

async fn check_fees(ctx: Context, channel_id: ChannelId, fee_threshold: u64) {
    info!("La tâche de vérification des frais a démarré.");

    let _api_url = "https://mempool.space/api/v1/fees/recommended";
    let mut last_notified_low = false;
    let mut last_notified_high = false;

    loop {
        info!("Vérification des frais...");
        match fetch_fee_data().await {
            Ok(data) => {
                let current_fee = data.fastest_fee;
                info!("Frais actuels : {} sat/vByte", current_fee);

                // Alerte pour frais inférieurs ou égaux au seuil
                if current_fee <= fee_threshold && !last_notified_low {
                    let alert_message = format!(
                        "⚠️ Les frais de transaction Bitcoin sont maintenant à {} sat/vByte, sous le seuil de {} sat/vByte!",
                        current_fee, fee_threshold
                    );
                    if let Err(e) = channel_id.say(&ctx.http, alert_message).await {
                        error!("Erreur lors de l'envoi du message d'alerte : {}", e);
                    } else {
                        last_notified_low = true;
                        last_notified_high = false; // Réinitialiser l'autre alerte
                        info!("Alerte envoyée pour les frais inférieurs à {} sat/vByte.", fee_threshold);
                    }
                }

                // Alerte pour frais supérieurs au seuil
                else if current_fee > fee_threshold && !last_notified_high {
                    let alert_message = format!(
                        "⚠️ Les frais de transaction Bitcoin sont maintenant à {} sat/vByte, au-dessus du seuil de {} sat/vByte!",
                        current_fee, fee_threshold
                    );
                    if let Err(e) = channel_id.say(&ctx.http, alert_message).await {
                        error!("Erreur lors de l'envoi du message d'alerte : {}", e);
                    } else {
                        last_notified_high = true;
                        last_notified_low = false; // Réinitialiser l'autre alerte
                        info!("Alerte envoyée pour les frais supérieurs à {} sat/vByte.", fee_threshold);
                    }
                }
            }
            Err(e) => {
                error!("Erreur lors de la désérialisation des données : {}", e);
            }
        }

        // Attendre 10 minutes avant la prochaine vérification
        sleep(Duration::from_secs(600)).await;
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

    // Définir les intents nécessaires pour recevoir les événements
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES;

    // Définir le seuil de frais pour les alertes
    let fee_threshold = 5;

    // Créer le client avec le gestionnaire d'événements
    let handler = Handler {
        channel_id,
        fee_threshold,
    };

    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
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
