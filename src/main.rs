use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use reqwest;
use serde::Deserialize;
use std::env;
use tokio::time::{sleep, Duration};
use dotenv::dotenv;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("Connecté en tant que {}", ready.user.name);
        
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

    let channel = match ctx.http.get_channel(channel_id).await {
        Ok(channel) => channel,
        Err(e) => {
            println!("Erreur lors de la récupération du canal : {}", e);
            return;
        }
    };

    let channel_id = match channel {
        serenity::model::channel::Channel::Guild(c) => c.id,
        _ => {
            println!("Le canal spécifié n'est pas un canal de serveur.");
            return;
        }
    };

    let api_url = "https://mempool.space/api/v1/fees/recommended";
    let fee_threshold = 2;
    let mut last_notified = false;

    loop {
        match reqwest::get(api_url).await {
            Ok(response) => {
                match response.json::<FeeData>().await {
                    Ok(data) => {
                        let current_fee = data.fastestFee;
                        println!("Frais actuels : {} sat/vByte", current_fee);

                        if current_fee <= fee_threshold && !last_notified {
                            if let Err(e) = serenity::http::Http::new(&env::var("DISCORD_TOKEN").unwrap())
                                .send_message(channel_id.into(), |m| {
                                    m.content(format!(
                                        "⚠️ Les frais de transaction Bitcoin sont maintenant à {} sat/vByte!",
                                        current_fee
                                    ))
                                })
                                .await
                            {
                                println!("Erreur lors de l'envoi du message : {}", e);
                            } else {
                                last_notified = true;
                            }
                        } else if current_fee > fee_threshold && last_notified {
                            // Réinitialise la notification lorsque les frais remontent au-dessus du seuil
                            last_notified = false;
                        }
                    }
                    Err(e) => {
                        println!("Erreur lors de la désérialisation des données : {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Erreur lors de la requête à l'API : {}", e);
            }
        }

        // Attendre 5 minutes avant la prochaine vérification
        sleep(Duration::from_secs(300)).await;
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Récupérer le token Discord depuis les variables d'environnement
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN non définie dans le fichier .env");

    // Créer le client
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::DIRECT_MESSAGES;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Erreur lors de la création du client");

    // Démarrer le client
    if let Err(why) = client.start().await {
        println!("Erreur du client : {:?}", why);
    }
}
