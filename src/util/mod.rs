pub fn get_leaf_folders(folders_to_create: Vec<&str>) -> Vec<String> {
    let mut sorted_folders = folders_to_create.clone();
    sorted_folders.sort();

    let mut leaf_folders = Vec::new();

    for i in 0..sorted_folders.len() {
        let mut is_leaf = true;
        for j in (i + 1)..sorted_folders.len() {
            if sorted_folders[j].starts_with(sorted_folders[i]) {
                is_leaf = false;
                break;
            }
        }
        if is_leaf {
            leaf_folders.push(sorted_folders[i].to_string());
        }
    }

    leaf_folders
}
